# ADR-345: Per-link CSI attribution and node-to-node ranging

- **Status**: Proposed — measurement layer implemented and validated on hardware; localization not built
- **Date**: 2026-08-28
- **Deciders**: Joe
- **Tags**: csi, wire-format, esp32-c6, localization, adr-018, adr-060, adr-110

## Context

Every attempt at device-free position estimation in this repository has failed
against the same wall, and the wall has now been measured twice from different
directions.

**Phase is unusable.** The bistatic tier needed a signed per-link radial
velocity, which requires the common-mode component of CSI phase. Measured
2026-08-28 across all three ESP32-C6 nodes over ~40,000 frames, the
frame-to-frame change in common-mode phase is statistically indistinguishable
from uniform random — resultant length `R` = 0.011–0.032 against a 0.000
reference, standard deviation 1.78–1.81 against `pi/sqrt(3)` = 1.814, and no
lag-1 autocorrelation. A second, unwrap-free estimator agreed. The mechanism is
packet-detection timing quantization: common-mode phase is `-2*pi*f_c*tau`, and
one ADC sample of jitter at 20 MHz is `2*pi * 2.412e9 * 50e-9` ~= 758 rad, about
120 full wraps, re-randomized every packet. Not calibratable. The bistatic tier
is default-off as a result (`bistatic_tier_enabled`).

**Amplitude works but had no parallax.** The three AP -> node links leave a
single distant transmitter within a 36.5 degree fan and converge on the node
cluster. Bearings computed from `room_config.json`: 198.5, 221.2, 235.0 degrees.
Scalar per-node energy weighting was closed with a clean negative result on
2026-08-27 — magnitude responds to proximity, direction does not.

**The receivers were hearing far more than we recorded.** A node in promiscuous
MGMT+DATA mode with no `filter_mac` produces CSI for every transmitter on the
channel. Measured on node 2: the AP accounted for only **34%** of frames; the
two peer nodes supplied 39% and 27%. Those peer frames are valid measurements
of *node-to-node* channels — but ADR-018's wire format carries no transmitter
address, so the sink could not attribute them. They interleaved into one
`frame_history` with mixed geometry, and the only available remedy
(`filter_mac`, ADR-060) fixes the mixing by discarding ~75% of the data.

The node-to-node links between the same three boards cross the room from three
sides at **108.7 degrees** of angular spread (bearings 0.0, 73.2, 108.7) versus
36.5 for the AP links — roughly 3x the parallax, with both endpoints known,
fixed, and under our control rather than wherever the router happens to live.
Link count also grows as `N*(N-1)/2`: 3 nodes give 3 links, 6 give 15, 13 give
78.

## Decision

Carry the transmitter MAC on the CSI wire and model CSI per **link**
(receiver, transmitter) rather than per **node**, as a layer alongside the
existing per-node model.

### 1. Wire format v2

`CSI_MAGIC_V2 = 0xC5110008`, header 26 bytes: the v1 header unchanged through
byte 19, then the transmitter address (`info->mac`, 802.11 addr2) at bytes
20..25, with I/Q from byte 26.

`0xC5110002`..`0xC5110007` were already claimed by the vitals, feature and other
edge packets, so v2 takes the next free value rather than an adjacent one — see
`issue_928_magic_collision_tests` for why that matters. The parser accepts both
magics, so a mixed fleet keeps working and v1 nodes simply report no
transmitter.

### 2. Per-link state (`links.rs`)

`LinkTable` keyed by `LinkId { rx_node, tx_mac }`, each link holding its own
amplitude history, RSSI EMA, learned resting baseline and motion metric.

The metric is the mean over subcarriers of the temporal standard deviation of
**AGC-normalized** amplitude. Removing each frame's own mean across subcarriers
is essential rather than cosmetic: the ESP32 applies automatic gain control per
packet, so absolute amplitude jumps between packets for reasons unrelated to the
channel, and any statistic built on the raw level measures the radio's gain
control instead of the room.

Amplitude only, deliberately — nothing here can depend on phase coherence.

### 3. Beacon rate

`BEACON_PERIOD_MS` 100 -> 20. At 10 Hz a peer samples a node-to-node link with a
5 Hz Nyquist while motion Doppler lives at 1–20 Hz; it aliases before it starts.
20 ms matches both the receiver's own `CSI_MIN_PROCESS_INTERVAL_US` ceiling and
the gateway self-ping already used for the AP link.

### 4. `GET /api/v1/links` and a live UI panel

The link table was initially reachable only through a 30-second log line, which
is the wrong medium for a quantity whose value is watching it respond while you
walk across the room. `GET /api/v1/links` returns one row per link — `motion`,
`raw_motion`, `rssi_dbm`, `frames`, `kind` — and `LinkMeshPanel` renders them in
the Sensing tab at 1 Hz. `raw_motion` is shown beside `motion` deliberately: a
link whose baseline has not settled reports a healthy raw value and a near-zero
motion, and with only the one number that is indistinguishable from a dead link.

Node identity is **inferred, not measured**. Nothing in the system records a
node's own MAC — a node reports the addresses it hears, never its own. But a
radio does not receive its own transmissions, so given the set of receivers
currently reporting, a transmitter heard by all of them except exactly one is
that one receiver's own board; a transmitter heard by every receiver is external
infrastructure. `links::infer_transmitting_node` declines (returns `None`) with
fewer than three receivers, where "heard by all but one" and "heard by exactly
one" are the same observation, and whenever more than one receiver is missing,
which a peer out of range produces indistinguishably. The field is named
`tx_node_inferred` and the UI draws the peer id with a trailing `?`.

Checked against the known MAC map on 2026-08-28 the inference labelled all four
transmitters correctly — nodes 0, 1 and 2 by address, and the AP as external —
with no access to that map. A wrong answer here mislabels a display row; it
never reaches a metric.

## Consequences

### Validated on hardware, 2026-08-28

All three nodes on wire v2 at 20 ms produced the full mesh: **9 links** (3 AP +
6 directional node-to-node).

Reciprocity — each physical channel measured independently from both ends:

| channel | direction A | direction B | ratio |
|---|---|---|---|
| 0 <-> 1 | 2.229 | 1.930 | 1.15 |
| 0 <-> 2 | 1.353 | 1.000 | 1.35 |
| 1 <-> 2 | 1.398 | 1.486 | 1.06 |

Nothing in the implementation enforces this. Three independent radio pairs
agreeing within 6–35% is the strongest available evidence that the per-link
separation is real rather than an artifact of frame bucketing.

Link quality is also far more uniform than the AP links: node-to-node raw motion
spans 1.0–2.2 across all six, versus 3.8–17.8 for the three AP links (which
track RSSI: -67, -72, -80). Uniform link quality is desirable for tomographic
localization — no single link dominates the solution.

### Known limitations

- **Peer links are ~10x weaker.** A 16-byte ESP-NOW beacon offers far less
  energy to estimate a channel response from than a full data frame. Whether
  1.0–2.2 perturbation is sufficient to localize with is **unknown and untested**.
- **Three links is below threshold.** Tomographic localization needs many
  crossing links; 3 will not localize regardless of quality. This ADR delivers
  the measurement layer, not localization.
- **Localization is not implemented.** No position tier consumes `LinkTable`.
  The live position estimate remains `doppler_centroid`.
- **Per-node path regression.** Removing `filter_mac` to admit peer frames
  returned the *per-node* `frame_history` to a mixed-transmitter stream, since
  it still consumes every frame regardless of source. Node 2's classification
  signal is noisier as a result. The fix is to select a link to feed the
  per-node path — software filtering that keeps the other links — and it is not
  yet done.

### Scaling limits to settle before more nodes

- **Airtime.** Every node broadcasts, so N nodes put N/period frames per second
  on the channel being sensed. 3 nodes at 20 ms is 150 pps; 13 would be 650 pps.
  `BEACON_PERIOD_MS` should become an NVS setting scaled to fleet size.
- **`MAX_LINKS` is 64.** 13 nodes give 78 receiver-transmitter pairs plus the AP
  plus neighbours, which overflows. Raise deliberately, with the memory implied.
- **Placement.** Links must *cross* the target area. Nodes spread through a
  house give roughly one link per room, each attenuated by walls and perturbed
  by anyone anywhere along it — whole-house coarse presence, not position.
  Perimeter placement around a single room is the geometry that works.

## Related bugs found and fixed while implementing

- **ADR-060 MAC filtering cost ~75% of capture rate.** The 50 Hz rate gate ran
  *before* the MAC filter and stamped `s_last_process_us` before the check, so a
  frame from any other transmitter consumed the slot and was then discarded.
  Enabling `filter_mac` dropped yield from ~42 pps to 6–13. Filtering first
  restores it (measured 31–37 pps). A 6-byte `memcmp` is negligible ISR work next
  to the CSI processing the gate guards.
- **The entire edge pipeline was dead on ESP32-C6.** `EDGE_MAX_SUBCARRIERS` was
  128, an S3-era assumption; a C6 on an HE-capable AP delivers HE20 frames with
  256 bins, and `process_frame` rejected every one. ADR-110 onboarded the C6 to
  the capture path but not this one. Vitals, presence, fall detection and
  per-slot counting had never run on these boards. Now target-conditional on
  `CONFIG_SOC_WIFI_HE_SUPPORT`, with the four subcarrier-sized arrays hoisted to
  static so peak stack is *lower* than it was at 128.
- **A target-conditional constant that failed silently.** `edge_processing.h`
  had no `#include` at all, so `CONFIG_SOC_WIFI_HE_SUPPORT` was undefined and
  `#if` evaluated it as 0 — selecting the wrong size with no warning. An
  undefined identifier in `#if` is 0 in C. This is the same failure mode as the
  bug it was fixing, one layer up.

## References

- `v2/crates/wifi-densepose-sensing-server/src/links.rs` — per-link state, 8 tests
- `v2/crates/wifi-densepose-sensing-server/src/phase_diag.rs` — the phase measurement
- `firmware/esp32-csi-node/main/csi_collector.c` — wire v2 serialization, filter ordering
- `firmware/esp32-csi-node/main/edge_processing.h` — `EDGE_MAX_SUBCARRIERS`
- ADR-018 (wire format), ADR-060 (MAC filtering), ADR-110 (C6 onboarding)
