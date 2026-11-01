# ADR-381: Node self-survey from pairwise ranging (check mode first)

- **Status**: Proposed
- **Date**: 2026-10-04
- **Deciders**: RuView maintainers; acceptance pending
- **Tags**: node-positions, ranging, ftm, uwb, multistatic, least-authority
- **Related**: ADR-018 (ESP32 wire format), ADR-110 (ESP-NOW time sync),
  ADR-141 (privacy control plane), ADR-144 (UWB range-constraint fusion),
  ADR-281 (BLE Channel Sounding ranging evidence), ADR-308 (placement
  optimizer), rufield ADR-261 (BLE evidence and Channel Sounding adapter
  boundary, `vendor/rufield/docs/ADR-261-ble-evidence-and-channel-sounding.md`)
- **Issues and PRs**: #1804, #1866, #2109 (show which nodes use the default
  position), #2133 (give the governed fuser the configured node positions)

## Context

Node positions reach the sensing server only through `--node-positions`. The
operator types it by hand, keyed by node id (`id:x,y,z;...`) or by list index
when the prefix is omitted. Three kinds of mistake pass silently today:

- **Wrong ids.** A config keyed by ids that do not match the live nodes looks
  applied and does nothing (#1804, #1866). #2109 makes the fallback to the
  default position visible, and #2133 makes sure both fusers receive the
  map. Neither can tell whether a correctly-keyed position is *right*.
- **Wrong units.** Positions measured in feet and typed as metres are 3.28×
  too large. Nothing checks the scale.
- **Stale positions.** A node moved after setup keeps its old coordinates.

Each one degrades multistatic fusion without an error. The repository has no
node-to-node ranging and no anchor-free solver. ADR-144 models
anchor-to-*tag* UWB ranges for people tracking
(`wifi-densepose-mat/src/localization/range_constraint.rs`), not ranges
between the fixed nodes themselves. ADR-281 computes BLE Channel Sounding
ranging evidence, but there is no CS-capable radio in the fleet: rufield
ADR-261 notes that the ESP32-S3 does not implement CS.

The ESP32-S3 and C6 nodes do support Wi-Fi Fine Timing Measurement (802.11mc
RTT, station initiator ↔ SoftAP responder) in mainline ESP-IDF. So a ranging
source exists on hardware we already have. The question is how good it is.

### What FTM measured

Bench, 2026-10-04 (UTC): two ESP32-S3 boards (rev v0.2), ESP-IDF v5.5
`examples/wifi/ftm`, SoftAP responder on channel 1 at 20 MHz, 32 FTM frames
per session, line of sight, tape ground truth. All rows MEASURED; the
reproducer is in Appendix A.

| Tape (m) | Result |
|---|---|
| 0.00 | median 0.00 m |
| 0.76 | median 0.00 m. Readings clamp to 0 below about 0.8 m. The first three sessions after a responder start read 1.8 to 3.0 m |
| 1.14 | five responder starts: medians 0.60, 1.50, 2.25 and 3.08 m, plus a 7.7 m run at −68 dBm with a 1.92 m session spread (placement outlier) |
| 3.71 | median 7.80 m (one start) |

- **Precision within one responder start:** ±0.2 to 0.4 m (standard
  deviation 0.19 to 0.31 m). A 40-session run over about two minutes showed
  no drift beyond that.
- **Bias between responder starts:** −0.5 to +1.9 m at 1.14 m, with the
  boards untouched. A plain-versus-auto-start responder A/B test showed the
  responder build is not the cause.
- **Quantisation:** every distance is a multiple of 0.15 m (1 ns of RTT).
- **Not tested:** 40 MHz, other channels, `ftm -R -o` offset calibration, a
  second pair, non-line-of-sight, CSI coexistence, raw t1..t4 timestamps.

FTM is precise inside one start and inaccurate across starts. It can catch
mistakes worth metres. It cannot place a node to decimetres. UWB (Qorvo
DW3000 family) is the expected accuracy path: about 0.1 m line of sight is
CLAIMED by the vendor, and we have no measurement of our own yet.

## Decision

### 1. A source-agnostic `RangeObservation` record

```text
RangeObservation {
    initiator: u8,             // node_id, as in --node-positions
    responder: u8,
    range_m: f64,              // session median
    sigma_m: f64,              // session spread, 0 if unknown
    method: Ftm | Uwb | BleCs | Manual,
    samples: u16,              // valid frames / exchanges
    rssi_dbm: Option<i16>,
    bandwidth_mhz: Option<u16>,
    at_us: u64,                // host receive time
    responder_boot: Option<u32>,  // boot counter or nonce of the responder
    calibrated: bool,          // set only by the host calibration step
}
```

`responder_boot` is required for calibration. The FTM bias moved with every
responder start, so an offset is only valid within the boot it was measured
in.

On the wire, nodes report ranges either in a new ADR-018-style packet with
its own magic, or as an extension of the existing sync packet. This ADR does
not choose between them. The host-side record and solver come first. The
wire format follows the first firmware that emits ranges.

### 2. Host-side check mode (warn-only)

Given the configured positions and a batch of observations, check mode
reports:

- **Per-link residuals.** `measured − ‖p_a − p_b‖`, with a tolerance of
  `floor + k·σ`. σ is the larger of the median session sigma and the spread
  across sessions. Each link is `ok`, `outlier`, `too_short` (configured
  length below the source's minimum), `weak` (every session below the RSSI
  gate), or `noisy` (σ above the source's maximum).
- **Unit mismatch.** A least-squares scale `s` in `measured ≈ s·expected`.
  If `s` sits within a band of 0.3048 (feet), 0.0254 (inches), 0.01 (cm) or
  0.001 (mm), and rescaling removes outliers, the check reports the unit.
- **Id swap.** For up to 8 ranged nodes, it scores every assignment of the
  configured positions to ids. The cost is a truncated quadratic in residual
  over tolerance, so one bad link cannot dominate. If another assignment
  cuts the cost to at most half and the configured cost is material, the
  check suggests that assignment, reported as `(node, position_of)` pairs.
- **Moved node.** If most of a node's links are outliers (at least 2, and a
  strict majority), the node was probably moved. Outlier links left over are
  reported one at a time as possible bias, a body or wall in the path, or a
  bad session, and not as a configuration error.
- **Unknown and unranged nodes.** Observations naming unconfigured ids, and
  configured nodes that no scored link reaches.

Distances do not change under rotation, reflection or translation, so check
mode compares distances and never coordinates. Where the layout is
symmetric, it says so instead of guessing:

- A relabelling that is a symmetry of the layout is **invisible**. For
  example, ids rotated 180° in a rectangle of corner nodes. Check mode
  reports such a layout as consistent.
- When several assignments fit equally well, check mode prefers the one that
  changes the fewest ids, since a typo moves few. It reports how many
  assignments tied. If the fewest-changes assignments are themselves tied,
  as with a swap along one wall of a rectangle (its mirror image is the
  opposite wall's swap), it reports `id_swap_ambiguous` and names nothing.

Corner-mounted nodes in a rectangular room are the common case and are
nearly symmetric. Operators should expect ambiguity reports there, and a
fifth node off the axes removes most of them.

**Authority.** Check mode never writes positions, never edits config, and
never changes fusion. Its output is a report for the startup log and a
read-only status field. The report shape is JSON-serialisable; the field
name follows #2109's `node_positions` block once that lands.

**Tolerances come from measured data:**

| Source | `floor_m` | `calibrated_floor_m` | `max_sigma_m` | `min_expected_m` | Basis |
|---|---|---|---|---|---|
| FTM, ESP32-S3, 20 MHz | 2.0 | 0.6 | 1.0 | 1.0 | MEASURED, bench above: bias −0.5 to +1.9 m between starts; spread ≤ 0.31 m within a start (the 1.92 m outlier excepted); clamp below about 0.8 m |
| UWB | 0.3 | 0.3 | 0.3 | 0.0 | ESTIMATE until our own UWB bench. Vendor accuracy is CLAIMED |

With `floor_m = 2.0`, the one 3.71 m start (+4.1 m) would be flagged as a
single-link outlier. That is uncalibrated FTM as it stands. Single-link
outliers stay low-severity on purpose, and the strong findings (unit, swap,
moved node) need several links to agree.

### 3. Per-boot calibration

An offset is recorded per `(initiator, responder, responder_boot)` from
sessions at a known reference distance of at least 1.0 m:
`offset = median(range) − reference`. It applies only to observations with
the same key. After a responder reboot, observations pass through
uncalibrated and get the wider floor.

Zero-separation calibration (ESP-IDF's `cm0` / `ftm -R -o`) is not used,
because short ranges clamp to 0 on this hardware. In practice, the reference
is one tape-measured link per responder per boot, or a UWB link once UWB
exists. That the bias is stable over a whole boot, and not only for two
minutes, is untested (ESTIMATE). Measuring it is the first FTM follow-up.

### 4. Fill mode, gated off

Fill mode (solve positions with classical-MDS initialisation, weighted
robust SMACOF and gauge fixing, then write a file the operator passes to
`--node-positions`) is **not implemented** and stays disabled for a source
until all of these hold:

- the source has **MEASURED** link-error evidence with a reproducer;
- the evidence covers at least 6 ground-truth links;
- the p90 link error is **≤ 0.3 m**.

FTM's measured errors are metres, so it fails this gate. UWB is expected to
pass it, but that is unproven. Even when the gate opens, fill mode writes a
proposal file. The server never rewrites its own positions.

### 5. Pluggable sources

Every source produces `RangeObservation`, and check mode does not care which
one did. Only the tolerance profile changes.

| Source | Status | How it plugs in |
|---|---|---|
| **FTM** (ESP32-S3/C6) | Now, for check mode | APSTA node runs a short scheduled survey window. One node is the SoftAP responder and the others initiate a directed session. `rtt_ns`, RSSI and session spread map to `range_m`, `rssi_dbm`, `sigma_m`. Airtime cost on the CSI channel is unmeasured |
| **UWB** (DW3000 family, e.g. DWM3001CDK over USB) | After its own bench | Two-way ranging between nodes. Shares hardware with ADR-144's anchor-tag model, but node↔node ranges feed this ADR and anchor↔tag ranges keep feeding ADR-144 |
| **BLE CS** | When a CS-capable radio exists | Through the rufield ADR-261 adapter boundary, using ADR-281's cross-validated phase/RTT ranging. A `Divergent` anomaly drops the observation. Single-mechanism evidence gets the wider floor |
| **Manual** | Now | A taped distance entered by the operator. Mainly useful as the per-boot calibration reference |

### 6. Prototype

`v2/crates/ruview-survey` implements sections 1 to 4 on the host, with no
I/O, no clock and no randomness, and bounded inputs: 64 nodes, 10 000
observations, 100 m range, and the swap search capped at 8 nodes.
Integration with the sensing server is a follow-up and depends on #2109 for
where the report surfaces. The crate's tests are SYNTHETIC: generated layouts
with FTM-like bias and noise from the envelope above. They cover consistent
configs, feet and centimetre mistakes, a two-id swap, a three-id rotation,
symmetric invisibility and ambiguity, a moved node, a bench-style single-link
outlier, a noisy link, per-boot calibration, and the fill gate.
`examples/check_ranges.rs` runs check mode on a JSONL file.

## Consequences

- An operator gets a warning when ranges and `--node-positions` disagree
  grossly, with no new authority: check mode is read-only.
- FTM on nodes needs APSTA and a SoftAP beacon on the sensing channel. Its
  cost to CSI yield must be measured before the firmware side ships.
- The FTM tolerances rest on one evening with one pair. They are parameters,
  not constants, and must be revised from more data (40 MHz, more pairs,
  non-line-of-sight).
- Ranges between fixed nodes are not person data. Ranges to phones or tags
  are, and remain under ADR-141 and ADR-144.
- A symmetric layout can hide a relabelling entirely. Check mode documents
  this and does not claim to catch it.

## Evidence rule

No self-survey accuracy claim without a MEASURED reproducer: tape or laser
ground truth, at least 4 nodes, line of sight plus at least one
non-line-of-sight case, every number tagged. The prototype's tests are
SYNTHETIC and show logic, not radio performance.

## Alternatives considered

- **Treat FTM as accurate enough to fill positions.** Rejected: the measured
  bias between starts (−0.5 to +1.9 m at 1.14 m) and the +4.1 m at 3.71 m
  rule it out.
- **Drop FTM.** Rejected for now. It costs nothing on existing hardware, it
  catches metre-scale mistakes, and it may suit coarse gating, calibrated
  pairs within a boot, and time sync. It is parked and revisited after UWB.
- **Zero-separation `cm0` calibration.** Rejected: readings clamp to 0 below
  about 0.8 m.
- **Server auto-corrects positions.** Rejected under least authority. Fill
  mode, when enabled, writes a proposal file only.
- **LoRa SX1280 ranging, router (MT7981) FTM, raw I/Q phase ranging,
  CSI-amplitude ranging.** Rejected or deferred: wrong distance regime, no
  driver support, unproven, or too inaccurate respectively.

## Open questions

- Does one calibration offset hold for a whole responder boot? For both
  roles? For other initiators against the same responder?
- Is the post-start transient (1.8 to 3.0 m in the first three sessions at
  0.76 m) real, and how long does it last?
- Does 40 MHz shrink the bias between starts?
- What does an FTM survey window cost in CSI frames on the same channel?
- Can FTM t1..t4 timestamps be tied to the CSI timestamp clock for sync?
- The wire format: new magic, or an extension of the sync packet?

## Appendix A: FTM bench reproducer

1. Flash ESP-IDF v5.5 `examples/wifi/ftm` on two ESP32-S3 boards
   (`CONFIG_ESP_WIFI_FTM_ENABLE=y`, the example's default).
2. Responder console: `ap <ssid> <password>` (channel 1, 20 MHz defaults).
   Use throwaway credentials and do not commit them.
3. Initiator console, at a taped separation: `ftm -I -s <ssid>`, 32 frames,
   10 to 40 sessions per distance. Record `Avg raw RTT`, `Avg RSSI`,
   `Estimated Distance` and the valid-frame count for each session.
4. Reset the responder and repeat at the same distance to see the bias move
   between starts.
5. Report the median, spread and error per run, tagged MEASURED, with the
   raw per-session data alongside.
