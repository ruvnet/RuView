# wifi-densepose-mtk-bridge

Bridges MediaTek MT7981 (`mt76`) vendor CSI into RuView's ADR-267 MTC1 frames and
sends them to the sensing server's UDP ingest (default `127.0.0.1:5005`).

Developed in whitsentry, a RuView-based project. User guide:
[`docs/mediatek-router-csi.md`](../../../docs/mediatek-router-csi.md).

MT7981B + MT7976C is the silicon in the Wavlink WL-WN586X3, and per the OpenWrt
hardware tables also in the OpenWrt One and the Xiaomi AX3000T (CLAIMED: only the
WN586X3 has been run). MediaTek's public OpenWrt feed ships a CSI driver
patch and a `mt76-vendor` userspace tool for it; `MtkWifiRev/MtkCSIdump` streams
the same data over UDP. This crate consumes either and speaks MTC1.

**Real silicon has been through this path** as of 2026-09-19: one CSI record
off a Wavlink WL-WN586X3 (MT7981B + MT7976C) decoded and reached the sensing
server as `mediatek:physical-unvalidated`. That record stays out of this
repository. It holds a client transmitter address and raw channel samples.
What it proved, and what the in-repo test locks, is the shape: legacy OFDM
(`rx_mode = 1`), 20 MHz, 64 subcarriers, one chain, the driver's last-chain
bit set. That is a path check, not a sensing-quality result.

## Input transports

**MtkCSIdump UDP datagrams** (`--listen`) — a 20-byte packed header
(`u64` millisecond timestamp, `u32 antenna_idx`, `u32 packet_count`,
`u32 total_samples`) followed by `{f64 i, f64 q}` sample pairs, in host byte
order. Samples are doubles converted from the driver's `s16`, so they narrow back
without loss. The sender trims edge subcarriers, so BW20/40/80/160 arrive as
61/125/253/509 of 64/128/256/512 and the bandwidth is recovered from `len + 3`.
A client must send the bytes `register` before the server forwards anything,
which `--csidump <host:port>` does.

This transport is lossy by design. It carries no RSSI, SNR, TA, transmit index,
bandwidth code, PPDU mode or sequence number, and **no packet identity**:
`packet_count` is always 1 and `timestamp` is taken per datagram at send time.
CSIdump's send loop is also antenna-major — every packet of antenna 0, then every
packet of antenna 1 — so chains of one PPDU arrive far apart with different
timestamps. Nothing can correlate them, so **the bridge emits one 1x1 frame per
datagram**. Pairing by arrival position would desynchronise silently on any UDP
loss and splice chains from unrelated PPDUs, which is worse than reporting what
each datagram is. Use the dump-file path when MIMO structure matters.

Fields the datagram lacks are reported as absent, not as zero: `rssi_dbm` and
`noise_floor_dbm` carry the sentinel −128 dBm, which is below any receiver's noise
floor and so cannot be mistaken for a measurement (0 would read as a very strong
signal), and `ppdu_type` is `Unknown`. Provenance is unaffected: a live capture is
still `mediatek:physical-unvalidated`, and a replayed one needs `--captured-on`.

**`mt76-vendor dump csi <n> <file>` JSON** (`--replay`) — an array of 13-element
records: `ts`, `ta` hex, `rssi`, `snr`, `data_bw`, `pri_ch_idx`, `rx_mode`,
`tx_idx`, `rx_idx`, `chain_info`, `ext_info`, the I array, the Q array. The
vendor tool appends to its output file (`fopen(name, "a+")`), so repeated dumps
concatenate top-level arrays; the reader streams them. These records are untrimmed
and carry per-chain RSSI and a real transmit index, so prefer this path when MIMO
structure matters.

Accepting concatenated arrays is deliberate, because a file the vendor tool wrote
twice legitimately holds two. It is also a foot-gun: a capture loop that does not
remove the file between dumps replays every earlier dump, multiplying records
downstream. Parsing cannot tell the two apart, so the bridge reports both signals
instead of guessing — a note when a file holds more than one array, and a
**warning** counting records that repeat a `(ta, ts, tx_idx, rx_idx)` chain, which
is what an accumulated file looks like. Neither rejects the input.

Records are grouped into frames by the driver's own last-chain marker,
`chain_info & BIT(15)`, accumulated per transmitter address. Dimensions come from
the chains each run contained, so HT closes as 2x2 and legacy single-stream as
1x2 on the same radio, with no configuration. A chain repeating before the marker
abandons that run rather than splicing two PPDUs together.

A 14th element is accepted as `pkt_sn`. See "Sequence numbers" below.

## Provenance

The sensing server derives a `source` label from each frame's flags:

| Frame flags | Label |
|---|---|
| `SYNTHETIC` (whatever else is set) | `mediatek:simulated` |
| not synthetic, `CALIBRATED` | `mediatek` |
| not synthetic, not calibrated | `mediatek:physical-unvalidated` |

`mediatek:physical-unvalidated` must be reachable only from a real device. This
crate never sets `CALIBRATED` — nothing reaching it has been calibrated — and
sets `SYNTHETIC` only through the rules here.

A file on disk cannot prove it came off silicon, so **`--replay` refuses to start
unless the operator decides explicitly**:

- `--synthetic` flags every emitted frame `SYNTHETIC`.
- `--captured-on <model>/<firmware>` attests the file came off hardware. Both
  halves must be non-empty. The attestation is logged at startup.

An input can also **declare itself synthetic**, and that beats any attestation:
passing `--captured-on` for such an input is an error, not an override. Three
signals are honoured, any one sufficient:

1. A sidecar `<file>.provenance.json` holding
   `{"synthetic": true, "generator": "..."}`.
2. A `.synthetic.` infix in the file name, so a lost sidecar cannot silently
   downgrade a fixture.
3. A capture header whose `synthetic` field is true, so a recording made under
   `--synthetic` stays synthetic when replayed.

Synthetic is sticky: these signals can add it and nothing clears it.

The live `--listen` path is physical by construction, so it is not gated on a
declaration — it is gated on **where the datagram came from**. `--listen-allow
<cidr>[,<cidr>]` is mandatory: without it the bridge refuses to listen rather
than trusting whatever reaches the socket, and a datagram from a source outside
the list is dropped and counted as `rejected_source`, never decoded or
assembled. Entries are IP or IP/prefix, repeatable and comma-separated, and
loopback is **not** implicit — name `127.0.0.1` to accept a local sender. This
follows the sensing server's own ADR-296 UDP allowlist, with the difference that
an empty list here means refusal rather than "inactive".

The path prints `PHYSICAL-UNVALIDATED: source <ip>, device <id>` on its first
admitted frame, so a live run's provenance is visible in the log. `--synthetic`
is still accepted there for exercising a fabricated sender.

`fixtures/` holds fabricated inputs only. `FABRICATED-*.synthetic.json` is
hand-generated and carries both the infix and a sidecar, so it cannot be
replayed as physical. A file from a real radio is replayed with
`--captured-on <model>/<firmware>` and must not carry a synthetic marker.

## Frame mapping

| MTC1 field | Source |
|---|---|
| `chipset` | `Mt7981Mt7976` |
| `tx_count` / `rx_count` | `--tx-chains` / `--rx-chains`, default 2x2 |
| `subcarrier_count` | samples carried (256 for a BW80 dump, 253 over UDP) |
| `bandwidth_mhz` | `data_bw` code, else recovered from the array length |
| `subcarrier_spacing_hz` | `bandwidth / subcarriers` = 312 500 Hz |
| `ppdu_type` | `rx_mode` via `mt76`'s `enum mt76_phy_type`; pre-HT becomes `Legacy`, unrecognised becomes `Unknown`, never dropped |
| `rssi_dbm` | per-Rx-chain `rssi` |
| `noise_floor_dbm` | `rssi - snr` |
| `timestamp_us` | vendor `ts`, unwrapped across its u32 rollover |
| `device_id` | `--device-id` hex, else an FNV-1a 64 hash of `--node` |
| `scale` | `1.0` — raw firmware s16 units, nothing calibrated |
| `SATURATED` | any sample on an i16 rail |
| `TIME_SYNCHRONIZED` | only with `--time-sync` |
| `SYNTHETIC` | only via the provenance rules above |
| `CALIBRATED` | never set |

A vendor record covers one `(tx_idx, rx_idx)` chain of one PPDU, so several are
folded into each MTC1 frame. Groups that never complete are **dropped and
counted**, never zero-filled. Two receivers must use different `--node` values;
the device id derives from it.

### PPDU formats

`rx_mode` is `enum mt76_phy_type`. MediaTek's CSI driver treats legacy OFDM as a
first-class mode — its tone-mask `mode_map` is keyed by `MT_PHY_TYPE_OFDM` — and
the first real capture off a WN586X3 reported exactly that. So pre-HT modes (CCK
and OFDM) map to `PpduType::Legacy`, enum gaps and future values map to
`PpduType::Unknown`, and the mapping never fails. Dropping a frame because its
PPDU format predates HT would discard real measurements.

`Legacy` and `Unknown` are additions to the ADR-267 codec (discriminants 6 and 7).
A decoder older than that change rejects them as `UnknownPpduType`.

### Sequence numbers

`pkt_sn` exists in the firmware event and in `struct csi_data`, but no stock
public userspace path exposes it — the vendor tool neither parses nor prints it,
and the UDP datagram has no field for it. So:

- `pkt_sn` present → `sequence = pkt_sn`, and a delta other than 1 sets
  `DROPPED_PREDECESSOR`.
- `pkt_sn` absent → `sequence` is a local counter and `DROPPED_PREDECESSOR` is
  **never** set, because loss cannot be observed.

## CLI

| Flag | Meaning |
|---|---|
| `--listen <addr>` | Bind for MtkCSIdump datagrams. Exclusive with `--replay`. |
| `--csidump <addr>` | Send `register` to a running CSIdump server so it forwards to us. |
| `--listen-allow <cidr>` | Source networks allowed to feed `--listen`. Required with it. |
| `--replay <file>` | Replay an `mt76-vendor` JSON dump or a `.mtkcap` capture. |
| `--record <file>` | Write decoded records to a capture for later replay. |
| `--sink <addr>` | Sensing server MTC1 ingest. Default `127.0.0.1:5005`. |
| `--node <name>` | Node name, hashed into a stable device id. Default `mtk-node`. |
| `--device-id <hex>` | Explicit 64-bit device id, overriding `--node`. |
| `--center-freq-khz <n>` | Centre frequency of the monitored channel. |
| `--tx-chains <n>` / `--rx-chains <n>` | Expected MIMO dimensions for a dump replay. Default 2 and 2. Ignored on the UDP path, which emits 1x1 per datagram. |
| `--time-sync` | Assert a disciplined clock. Sets `TIME_SYNCHRONIZED`. |
| `--synthetic` | Declare the input fabricated. Sets `SYNTHETIC`. |
| `--captured-on <model>/<firmware>` | Attest a replayed file came off hardware. |
| `--replay-hz <n>` | Pace replay. `0` sends as fast as possible. Default 20. |
| `--max-frames <n>` | Stop after n frames. `0` runs until interrupted. |
| `--dry-run` | Decode and assemble but send nothing. |

```bash
cargo build -p wifi-densepose-mtk-bridge
B=./target/debug/wifi-densepose-mtk-bridge

# Fabricated fixture. --synthetic is mandatory: its sidecar declares it.
$B --replay crates/wifi-densepose-mtk-bridge/fixtures/FABRICATED-mt76-vendor-dump-mt7981-bw80.synthetic.json \
   --sink 127.0.0.1:5005 --node wn586x3-livingroom --synthetic

# Live capture off a router running `./CSIdump phy0-sta0 100 8888`.
# --listen-allow is mandatory; name the router's address (or its subnet).
$B --listen 0.0.0.0:8888 --csidump <router-ip>:8888 --listen-allow <router-ip> \
   --sink 127.0.0.1:5005 --node wn586x3-livingroom \
   --record captures/livingroom.mtkcap

# Replay that real capture: attest instead of declaring synthetic
$B --replay captures/livingroom.mtkcap --sink 127.0.0.1:5005 \
   --node wn586x3-livingroom --captured-on WN586X3/OpenWrt-24.10.8
```

Aim fabricated replays at an isolated sensing-server instance, not at a shared
demo-facing one — the server caches the latest frame and serves it until the next
arrives.

## Tests

```bash
cargo test -p wifi-densepose-mtk-bridge
cargo clippy -p wifi-densepose-mtk-bridge --all-targets -- -D warnings
```

`tests/provenance_gate.rs` pins the file-input rules, including that a hand-built
MtkCSIdump datagram still yields a physical frame while no file input can.
`tests/listen_gate.rs` pins the live path: it refuses to start without an
allowlist, drops and counts a source outside one without ever assembling it, and
admits a named source.

## Further reading

The OpenWrt patch set for a WL-WN586X3 (Rev A, OpenWrt 24.10.8) is
`firmware/openwrt-wn586x3/`. It is the driver and the two userspace fixes.
It is not an image, and it does not include captured CSI.
