# ESP32 firmware release qualification, 0.8.15, ESP32-S3

This record exercises the release qualification harness on real hardware and
reports what it measured. It is the evidence for the procedure the harness
encodes, and it is deliberately not a pass: the transport path is clean on
every error criterion, and the raw CSI yield does not reach the threshold
that the 2026-08-31 record established for a different board, access point,
and room.

## Scope

This run qualifies the **transport path** of firmware 0.8.15 on one
ESP32-S3 node. It makes no occupancy, sensing-accuracy, or edge-DSP claim.
The edge pipeline on this node reported a 9.26 Hz mean DSP rate, which is not
the Tier 0 raw-passthrough configuration that the 2026-08-31 record
qualified, so the two runs are not comparable on the yield axis.

The node is one of two variants the repository ships. The ESP32-C6 variant
is not covered here: the C6 qualification in the 2026-08-31 record was made
against firmware 0.8.8, and this run has no C6 board.

## Hardware, firmware, and environment

| Field | Value |
|---|---|
| Board | ESP32-S3 DevKitC-1 N16R8 |
| Logical node | 1, at 192.168.0.107 |
| Firmware under test | 0.8.15 |
| Access point | 192.168.0.1, channel 6 |
| Sensing server | the same host, 192.168.0.110, over the LAN |
| Traffic source | `scripts/csi-udp-poke.py` at 50 Hz toward the node |
| Observation window | 300.0 s |
| Room | not characterised |
| Date | 2026-10-23 |

The traffic source is the documented way to give a node something to observe
when the access point answers ICMP at 802.11b rates and carries no CSI on
its own. Without it the same node reported 4 through 10 pps; with it, 4
through 27 pps. The traffic source is therefore part of the environment, and
it is named here.

## Result

<!-- ruview:esp32-firmware-qualification version=0.8.15 node=1 verdict=FAIL -->

| Criterion | Measured | Required | Result |
|---|---:|---:|---|
| raw CSI yield | 10.05 pps over 302 samples | at least 20.0 pps | FAIL |
| device send failures | 0 | 0 | PASS |
| device ENOMEM back-offs | 0 | 0 | PASS |
| device watchdogs | 0 | 0 | PASS |
| unexpected restarts | 0 | 0 | PASS |
| node frame coverage | 100.0 percent | at least 99.0 percent | PASS |
| source offline frames | 0 | 0 | PASS |
| JSON parse errors | 0 | 0 | PASS |
| WebSocket errors | 0 | 0 | PASS |
| early closes | 0 | 0 | PASS |
| observation window | 6425 frames in 300.0 s | a non-empty observation | PASS |

Server observation: 6425 frames in 300.0 s, maximum frame gap 148.9 ms.
Device observation: 302 controller ticks, raw yield 10.05 pps (range 4-27),
dsp 9.26 Hz, 1 ticks in the controller's degraded state.

Environment: ESP32-S3 DevKitC-1 N16R8, logical node 1 at 192.168.0.107,
firmware 0.8.15; access point 192.168.0.1 on channel 6; sensing server on
the same host at 192.168.0.110 over the LAN; bounded LAN UDP traffic source
`scripts/csi-udp-poke.py` at 50 Hz toward the node, because the access point
answers ICMP at 802.11b rates and yields no CSI on its own; the room was not
characterised, this run qualifies the transport path only.

Result: **FAIL**

The device counters are the change across the window, not the absolute
values: the node's send-failure counter was already at 96 when the capture
started, and it did not move.

## What the result means

**The transport path is clean.** Every error criterion is zero: no send
failures during the window, no ENOMEM back-offs, no uplink watchdog trips, no
restarts, no source-offline frames, no parse errors, no WebSocket errors, no
early closes. The node delivered 6,425 sensing frames in 300 seconds with
100 percent of them carrying node 1.

**The yield criterion is not met in this environment.** The threshold is the
one the 2026-08-31 record wrote down after a 300 second burn on an ESP32-S3
QFN56 module: at least 20 pps, measured at 28.03 pps on that board, and
34.92 pps on the C6 of the same date. The board here is a DevKitC-1 on a
different access point, with a different edge configuration, in an
uncharacterised room, and it sustained 10.05 pps.

**This run does not qualify firmware 0.8.15 as a stable release.** The
readiness gate agrees: `scripts/check-firmware-release-readiness.py
--version 0.8.15` reports the qualification as a failure. The gap between
10.05 pps and 20 pps is not explained by a firmware error that the run
observed, and this record does not claim one.

**The finding is that the thresholds are environment-bound and the
procedure was not runnable.** Before this change, the acceptance procedure
existed only as prose inside the 2026-08-31 validation record. There was no
script that ran it, no gate that checked whether a version had been
qualified, and no place that recorded the environment a number came from.
A threshold measured on one board could not be reproduced on another, and
nothing in the tree would have said so.

Three consequences follow, and they are the reason the harness and the gate
exist:

1. the yield threshold has to be declared per run
   (`--min-yield-pps`), and the environment has to be declared with it
   (`--environment-note`), because a number without its environment is not
   evidence;
2. a stable release needs a qualification that was produced by the harness
   and committed with its report, because the gate reads the report's
   version-scoped verdict rather than a prose claim;
3. a version whose qualification failed is blocked by the gate, which is
   what this record demonstrates end to end on the real tree.

## Reproducer

```bash
# the node console capture, for the length of the burn
# (any serial tool will do; the harness only reads the text)

# the traffic source
python scripts/csi-udp-poke.py 192.168.0.107 --hz 50 --seconds 305

# the qualification
python scripts/qualify-esp32-firmware.py \
    --node 1 --seconds 300 \
    --console-log capture.log \
    --firmware-version 0.8.15 \
    --environment-note "ESP32-S3 DevKitC-1 N16R8, node 1 at 192.168.0.107, \
firmware 0.8.15, access point 192.168.0.1 on channel 6" \
    --out qualification.json --print-md
```

The console capture and the JSON result are not committed: the capture is
raw device data and the result is reproducible from the commands above.

## Limits

- One board, one variant, one room, one access point. Nothing here
  generalises to the ESP32-C6 or to another deployment.
- The room was not characterised, so the run says nothing about occupancy.
  The node reported a non-zero motion score throughout, which was not
  investigated and is not claimed either way.
- The edge pipeline ran at about 9 Hz on this node. The 2026-08-31
  qualification was a Tier 0 raw-passthrough run at 20 Hz, and this record
  makes no claim about the edge DSP.
- The controller reported one tick in its degraded state out of 302. That
  state means the node decided its own radio was not delivering, which is
  consistent with the low yield. The acceptance criteria of the 2026-08-31
  record do not list it, so it is reported here rather than scored.
- The CSI UDP send-failure counter is not on the console; the console
  carries the first five failures and the mesh counter, and the
  authoritative per-run value lives in the node's 60 s flash log, which
  needs a flash dump to read.

## See also

- `docs/firmware-release.md` -- the release procedure this run belongs to.
- `docs/validation/2026-08-31-esp32-s3-rate-aware-transport.md` -- the
  record that defines the acceptance procedure and the thresholds.
- `docs/validation/2026-10-20-esp32-csi-input-release-candidate.md` -- the
  record that states the prerelease policy.
- `docs/releases/v0.8.8-esp32.md` -- the model stable release note.

## References

- `scripts/qualify-esp32-firmware.py` -- the harness that produced the
  table above.
- `scripts/check-firmware-release-readiness.py` -- the gate that reads this
  record.
- `firmware/esp32-csi-node/main/adaptive_controller.c` -- the 1 Hz
  controller tick the harness parses.
- `firmware/esp32-csi-node/main/csi_collector.c` -- the send-failure and
  ENOMEM counters the harness parses.
- `scripts/csi-udp-poke.py` -- the bounded LAN traffic source.
