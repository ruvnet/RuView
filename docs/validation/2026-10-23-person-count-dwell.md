# Person count dwell — live validation, 2026-10-23

Status: the published person count stopped moving while the room stayed the
same. Measured on one ESP32-S3 node against the sensing server, before and
after the change, with an identical procedure in both windows.

This record covers the *stability* of `estimated_persons`. It does not claim
that the count corresponds to the true occupancy — see "What this does not
show".

Issue: #2058 (Active Person Count is incorrect during live ESP32 CSI
streaming).

## What was measured

The server publishes `estimated_persons` on every sensing update. Before this
change the published value came straight out of the activity score's
hysteresis: the hysteresis holds the *output* steady while the score sits
inside a band, but it cannot hold it when the score crosses the whole band on
its own, so every threshold crossing moved the published number.

The room classification already has a 1.5 s debounce. The count had none, so
the count could move while the classification it is derived from stayed put.

## Setup

- Node: ESP32-S3 DevKitC-1 N16R8, rev v0.2, 16 MB flash, 8 MB PSRAM,
  MAC `90:70:69:18:9b:d0`, firmware 0.8.15.
- Server: `wifi-densepose-sensing-server` built from this tree,
  `--source esp32 --udp-port 5005 --udp-bind 0.0.0.0 --udp-insecure-lan
  --bind-addr 127.0.0.1`.
- Scene: the node's own serial log reported `motion=0.00 presence=0.00`
  throughout both windows, and the server's published room classification
  stayed `present_moving` for the whole of the "before" window.
- Sampling: `GET /api/v1/sensing/latest` polled at 5 Hz for 200 s, recording
  only the aggregate scalars (count, classification, feature energies). No raw
  CSI, amplitude vectors or per-person data were read or stored (ADR-299).

## Method

Both windows used the same procedure, in this order:

1. stop the sensing server;
2. start it from the build under test;
3. restart the node through the firmware's serial onboarding protocol
   (`RUVIEW_HELLO_V1` / `RUVIEW_CONFIG_V1 ... KEEP KEEP`, which reboots the
   node and rewrites only `node_id`, `target_ip` and `target_port`);
4. start sampling immediately.

Restarting the node is the trigger: the node's cold start is what makes the
activity score cross the thresholds.

The "before" build is the unmodified `upstream/main` tree
(`fefb0f91`) built from the same worktree with the same command, so the two
binaries differ only by this change.

## Results

| | before | after |
|---|---|---|
| samples | 705 | 736 |
| window | 196.3 s | 199.6 s |
| published count values seen | 1, 2, 3, 4 | 1 (and "no count" while the room vote read absent) |
| **changes in the published count** | **142** | **1** |
| longest stable run | 16.7 s | 197.49 s |
| room classification | `present_moving` throughout | `absent` for 2.3 s, then `present_moving` |

The single change in the "after" window is the room vote moving from `absent`
to `present_moving` at t=2.33 s, after which the count stayed at 1 for the
remaining 197 seconds.

In the "before" window the count moved in bursts — `1 → 3 → 1` roughly every
second — reaching 3 and 4, which is more than a single occupant can produce:
the activity score saturates at one occupant, so those excursions are not a
person entering the room.

A second "before" window, taken with the server already running and only the
node restarted, gave the same shape: 731 samples, 166 changes, values 1 to 4,
longest stable run 29.6 s.

## Reproducer

```bash
# Sample one window (the server must already be running against a live node).
python scripts/sample-person-count.py --out before.jsonl --seconds 200

# Summarise one or more windows.
python scripts/analyze-person-count.py before.jsonl after.jsonl
```

The scripts read the published aggregate only. `scripts/analyze-person-count.py`
prints the count distribution, the number of published changes and the longest
stable run for each window.

## Software evidence

```
cargo test -p wifi-densepose-sensing-server --no-default-features room_person_count_tests
  -> 14 passed (6 new, 8 pre-existing)
```

The six new tests cover the first positive count being published at once, a
candidate that does not hold for the dwell window, a candidate that does, a
flapping candidate that never moves the count, the dwell timer restarting for
a new candidate, the immediate drop to zero, and the room path publishing a
node's new count only after it holds.

The whole crate was run as well: 603 passed, 1 failed. The failure is
`udp_tee::tests::forwards_datagrams_byte_for_byte`, a UDP loopback socket test
that fails identically on the unmodified `upstream/main` tree on this host
(`os error 10060`). It is a pre-existing Windows loopback failure, not a
regression; the hosted checks run on Linux.

`cargo check` reports no new warnings from this change.

## What this does not show

- **The count still does not mean "how many people".** The activity score it
  is derived from is not an occupancy estimator, so in an empty scene the
  count can settle on a non-zero value. The dwell removes the *movement*, not
  the bias. The eigenvalue occupancy path is a separate matter (#1940).
- **One node, one room, one scene.** The windows are 200 s each and cover the
  node's cold-start transient; they do not characterise a long-running
  deployment or a scene with people in it.
- **No accuracy figure is claimed anywhere.** The measurement is about the
  published value moving, not about occupancy correctness.

## References

- Issue #2058.
- `docs/TROUBLESHOOTING.md` section 12, "Person count changes while the room
  is unchanged".
- `v2/crates/wifi-densepose-sensing-server/src/main.rs`,
  `AppStateInner::dwell_person_count` and `PERSON_COUNT_DWELL_DURATION_SECS`.
