# ADR-106 — Full Complex CSI in WS + Managed-Ping Keepalive

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/main.rs`
(`NodeInfo` struct, `NodeState`, `udp_receiver_task`,
`csi_keepalive_task`, CLI `--csi-keepalive-pps`).

## Context

The operator's instruction: *"work without a model for now, but make
sure the sensors give us everything described in the parent repo so
the future model — and fine-motion detection right now — has full
signal."* Two gaps stood between the live deployment and that goal:

1. **WS NodeInfo carried only amplitude.** The 56-bin per-subcarrier
   `amplitude` vector was exposed, but the equally-important
   `phases` vector (radians, `atan2(Q, I)`) was parsed by
   `parse_esp32_frame` and then silently dropped. Vital-signs FFT on
   phase, MERIDIAN-style hardware normalization, and any future
   DensePose-class model expect the full complex `H[k] = A_k · e^{jφ_k}`.
2. **Raw CSI rate depended on an ad-hoc shell `ping`.** With nothing
   sending unicast traffic to the sensors, beacon-only rate dropped
   to ~0.3 fps — too slow even for breathing-band FFT. The operator
   was running `ping -i 0.05 192.168.0.101 &` by hand; if Mac switched
   network, it died.

## Decisions

### D1 — Expose phases + noise_floor + n_antennas + µs timestamp in `NodeInfo`

Four new fields, each `#[serde(skip_serializing_if = empty/zero)]` so
feature_state ticks (no raw CSI) stay slim:

```rust
phases:           Vec<f64>,    // atan2(Q, I), radians
n_antennas:       u8,          // RX antenna count
noise_floor_dbm:  i8,          // RX noise floor
timestamp_us:     u64,         // sensor-side µs timestamp
```

This is the same data we already parse out of `0xC511_0001` frames
in `parse_esp32_frame`; previously we threw `phases` away and never
even surfaced `noise_floor` to the WS envelope. Consumers
reconstruct the complex CSI with `H[k] = amplitude[k] · (cos(phases[k]) + j·sin(phases[k]))`.

### D2 — Per-node stash on `NodeState`

`NodeState` gains four new fields:
`latest_phases: Option<Vec<f64>>`, `latest_noise_floor: i8`,
`latest_timestamp_us: u64`, `latest_n_antennas: u8`. Populated on
every raw-CSI frame in the second raw-CSI path
(`udp_receiver_task` → raw CSI branch). `build_node_features` and
the raw-CSI SensingUpdate builder both read from this stash to
populate the new `NodeInfo` fields uniformly. Avoids carrying a
full per-subcarrier phase history buffer — we only need the most
recent vector for the UI / classifier; FFT consumers can build their
own window.

### D3 — Built-in keepalive via managed `ping` children

`csi_keepalive_task` async task:

1. Watches `NODE_ADDRS` (per-node sender address, populated on every
   recv_from via a cheap magic-byte peek).
2. For each known node, spawns one `ping -i <interval> <ip>` child
   process (`/sbin/ping` on macOS, `/usr/bin/ping` on Linux).
3. Re-spawns the child if it dies or if the sensor's IP changes
   (DHCP rotation).
4. Default rate `--csi-keepalive-pps 25` → `-i 0.040` for `ping`.
   `--csi-keepalive-pps 0` disables.

### D4 — Why ICMP, not UDP

We first tried a UDP-based keepalive (`sock.send_to(&[0], src_addr)`
to the sensor's ephemeral source port). On the operator's deployment
(ESP32-S3 + TP-Link WISP) it did **not** drive raw CSI: the sensor's
UDP stack rejected the closed-port packet before the CSI callback
fired in the WiFi RX path. ICMP echo bypasses user-space port logic
entirely — kernel WiFi RX handles it and the CSI callback fires
regardless of any listener.

Trade-off accepted: shelling out to `/sbin/ping` is platform-
specific. Linux containers must include `iputils-ping`; macOS has
`/sbin/ping` built-in. We probe both paths at startup. A pure-Rust
raw-socket ICMP would avoid the dependency but needs root /
`CAP_NET_RAW`.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/main.rs
  - struct NodeInfo            (+4 fields, helpers is_zero_*)
  - struct NodeState           (+4 latest_* fields)
  - static NODE_ADDRS          (per-node source address map)
  - fn csi_keepalive_task      (managed ping pool)
  - udp_receiver_task          (NODE_ADDRS populate via magic peek)
  - all NodeInfo {...} sites   (5 — populate new fields)
  - Args { csi_keepalive_pps } (CLI flag, default 25)
docs/adr/ADR-106-full-complex-csi-keepalive.md   (this)
```

Two implementation commits on the branch:

* `4daa2c9b` — D1 + D2 (WS struct, per-node stash, NodeInfo builders)
* `8489efe9` — D3 + D4 (keepalive task, NODE_ADDRS, CLI flag)

## Verified Acceptance

Live, server fresh-restart, no shell `ping` running:

```
boot:    CSI keepalive: 25 ICMP pkt/s/node (interval 0.040s)
boot:    keepalive: learned address for node 1 = 192.168.0.101:60492
boot:    keepalive: learned address for node 2 = 192.168.0.100:51664
+2 s:    keepalive: ping -i 0.040 192.168.0.101 for node 1
+2 s:    keepalive: ping -i 0.040 192.168.0.100 for node 2

WS sample (5 s):
  node 1: 67.6 Hz updates, 55.6 Hz amp-bearing raw CSI
  node 2: 67.6 Hz updates, 55.6 Hz amp-bearing raw CSI
```

NodeInfo per node now carries `amplitude[56]`, `phases[56]`,
`rssi_dbm`, `noise_floor_dbm=-91`, `n_antennas=1`, plus the
empty/zero-suppressed `timestamp_us` (FW doesn't yet emit it —
left as a 0 placeholder).

Sampling rate 55 Hz comfortably covers breathing band (0.1–0.5 Hz)
and heart-rate band (0.8–2 Hz) for FFT; with the phase vector now
on the wire, those FFTs can run on phase as well as amplitude,
which is more sensitive to chest-wall micrometric motion.

## Out of scope / open

* ✅ **FW-side µs timestamp** — closed in commit `b787f40a`. FW now
  appends `info->rx_ctrl.timestamp` (u32 LE) as 4 trailing bytes
  after I/Q data; server parses opportunistically (None for older
  FW). NodeInfo.timestamp_us now carries sensor monotonic µs when
  available, falls back to server SystemTime otherwise.
* **Per-frame antenna selection** when ESP32-S3 reports >1 antenna —
  current FW hard-codes `n_antennas=1` in `csi_collector.c`. Single-
  antenna deployments are unaffected.
* **TP-Link queue limits** — at 55 Hz × 2 nodes = 110 raw frames/s,
  plus 25 pings/s × 2 = 50 ICMP/s, all going through one consumer-
  grade AP. Watching for saturation. Reduce `--csi-keepalive-pps` if
  the AP starts dropping.
* **Channel hopping** (ADR-029) would give frequency diversity. Single-
  channel works fine for one room.

## References

* ADR-100 — gain lock (the stability baseline keepalive needs).
* ADR-101 — classifier (consumes phase via per-node amplitudes; future
  micro-motion detector will pull phase too).
* ADR-103 — persistent baseline (loaded at server boot, unaffected
  by keepalive rate).
* ADR-105 — no synthetic data (this ADR adds *more* real data, not
  more synthetic).
* [`docs/references/espectre-gap-analysis.md`](../references/espectre-gap-analysis.md)
  — phase-aware processing is a prerequisite for several open items.
