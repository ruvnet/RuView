# ADR-267: MediaTek MIMO CSI Wire Protocol

- **Status**: accepted
- **Date**: 2026-07-18
- **Deciders**: RuView maintainers
- **Tags**: mediatek, csi, protocol, rust, udp, replay

## Context

The MediaTek simulator, captured regression fixtures, and a future `mt76` agent
need one safe host-side representation. Copying an undocumented firmware layout
would couple RuView to a private ABI and make malformed kernel/network data risky.
MIMO CSI also requires explicit Tx/Rx/subcarrier dimensions and per-Rx-chain RSSI.

## Decision

Define `MTC1` version 1 as a little-endian, self-delimiting envelope:

- 72-byte fixed header with magic, version, report kind, total length, sequence,
  monotonic timestamp, device ID, chipset profile, frequency, bandwidth, flags,
  Tx/Rx dimensions, numeric format, PPDU type, subcarrier count, noise floor,
  scale, subcarrier spacing, calibration ID, and payload length.
- CSI payload begins with one signed RSSI byte per Rx chain, followed by
  `tx_count * rx_count * subcarrier_count` complex values in Tx-major,
  Rx-major, subcarrier-major order.
- Supported numeric formats are complex signed i16 and complex finite f32.
- Capability reports use bounded opaque TLVs until a public driver contract exists.
- CRC-32/IEEE covers header and payload; the final four bytes carry the checksum.
- One envelope maps to one UDP datagram, capped at the IPv4 UDP payload maximum
  of 65,507 bytes. Replay files prefix each envelope with a little-endian `u32`.
- Parsers reject unknown versions/types/formats, invalid dimensions/bandwidth,
  multiplication overflow, inconsistent payload lengths, non-finite floats,
  bad CRC, trailing datagram bytes, and frames above the cap.
- Flags distinguish calibrated, saturated, time-synchronized, dropped-predecessor,
  and synthetic frames. Synthetic provenance cannot be cleared by downstream code.

## Amendment 1 — PPDU types `Legacy` and `Unknown`

- **Date**: 2026-09-19
- **Status**: proposed. Demonstrated on one WL-WN586X3 capture. Not a maintainer acceptance.
- **Scope**: `PpduType` discriminants only. The 72-byte header layout, CRC,
  payload ordering, flags, and caps are unchanged.

### What changed

`PpduType` gains two variants:

| Value | Name | Meaning |
|---|---|---|
| 6 | `Legacy` | Pre-HT: 802.11b CCK or 802.11a/g OFDM |
| 7 | `Unknown` | A PPDU format the producer could not classify |

Values 1-5 (`Ht`, `Vht`, `HeSu`, `HeMu`, `Eht`) are untouched.

### Why

The original set assumed every CSI report would be HT or later. The first real
MT7981 capture disproved that. A Wavlink WL-WN586X3 (MT7981B + MT7976C) running
an OpenWrt 24.10.8 image built with the patches in `firmware/openwrt-wn586x3/`
reported `rx_mode = 1` (`MT_PHY_TYPE_OFDM`) for a client associated on 2.4 GHz
channel 6 at 20 MHz. The bridge had no way to express it and dropped the frame.

That was a real data loss, not a corner case. MediaTek's CSI driver treats legacy
OFDM as a first-class mode: `mt7915_vendor_csi_tone_mask` skips only
`MT_PHY_TYPE_CCK`, and its `mode_map` is keyed by `MT_PHY_TYPE_OFDM`, `_HT`,
`_VHT` and `_HE_SU`, giving OFDM its own tone-mask group
(`1001-wifi-mt76-mt7915-csi-implement-csi-support.patch:1070-1088`). Association
and management exchanges are legacy-rate by design, so a sensing deployment will
see these continuously.

`Unknown` exists so the enum's gaps (5-7 and 12 in `mt76_phy_type`) and future
modes are carried explicitly rather than guessed at or dropped. It also covers
transports that omit the field: the MtkCSIdump UDP datagram carries no `rx_mode`,
and reporting `Unknown` there is honest where defaulting to a concrete format was
not.

### Compatibility

- **Forward, not backward.** A decoder built before this amendment rejects
  discriminants 6 and 7 with `UnknownPpduType`, because the original parser
  rejects unknown types by design. `TryFrom<u8>` returns `Err` rather than
  panicking, so an old decoder fails safely — it just drops legacy frames, and
  does so silently. **Rebuild MTC1 consumers after this codec change.**
- **Scope is narrower than "the workspace".** `wifi-densepose-hardware` defines
  three independent `PpduType` enums — `csi_frame.rs` (ESP32),
  `qualcomm_csi.rs`, and `mediatek_csi.rs` — and this amendment touches only the
  MediaTek one. Neither of the others changes, so no other vendor bridge is
  affected. The complete set of `mediatek_csi::PpduType` consumers is:
  `wifi-densepose-hardware`'s own `mediatek-csi-sim` binary,
  `wifi-densepose-mtk-bridge`, and `wifi-densepose-sensing-server`, which renders the
  value with `{:?}` and so needed no code change. In particular the
  `ppdu_type > 3` bound in `bootstrap_baseline.rs` is fed from the **ESP32**
  enum via `CsiGridKey::from_frame(&Esp32Frame)`, not from MediaTek frames, so
  `Legacy` and `Unknown` cannot reach it.
- Because the variants were added rather than renumbered, and Rust requires
  exhaustive matches, the compiler locates any consumer that must be updated.
  Nothing outside the files above needed one.
- Producers must emit 6 only for frames that genuinely are pre-HT, and 7 only
  where the format is truly unclassified. Neither is a dumping ground for a
  mapping the producer has not done.
- The mapping from `rx_mode` to `PpduType` is now infallible, so no MTC1 producer
  may drop a frame on PPDU grounds.
- Unchanged: synthetic provenance still cannot be cleared by downstream code.

### Observed dimensions

The same capture assembled to 1x1x64. That is the client's behaviour, not a
protocol limit: the handset sent single-stream frames to a 2.4 GHz 20 MHz AP.
Expect 1x2 or 2x2 on 5 GHz, and size consumers for the chipset profile's
`max_chains` rather than for what one capture happened to show.

## Consequences

### Positive

- Deterministic simulator and future hardware use identical parsing and APIs.
- Explicit dimensions prevent ambiguous antenna or subcarrier interpretation.
- CRC, finite-value checks, and hard caps make network/replay ingestion robust.
- The format supports MT7981, MT7986, and MT7996 profiles without claiming their
  undocumented firmware layouts.

### Negative

- A translation/copy step is required from a future kernel report.
- Maximum-size Wi-Fi 7 matrices may need segmentation in a later protocol version.

### Neutral

- Version 1 models one link per report; MLO correlation is a future extension.
- Capability TLVs are intentionally conservative until hardware metadata is known.

## Links

- [ADR-266: MediaTek Filogic CSI platform](ADR-266-mediatek-filogic-csi-platform.md)
- [ADR-018: ESP32 binary CSI framing](ADR-018-esp32-csi-frame-protocol.md)
- [ADR-264: RTL8720F radar wire protocol](ADR-264-rtl8720f-radar-wire-protocol.md)
- Amendment 1 implementation: `v2/crates/wifi-densepose-hardware/src/mediatek_csi.rs`
  (codec) and `v2/crates/wifi-densepose-mtk-bridge` (`ppdu_from_rx_mode`). The
  motivating capture is a lab record and is not in this tree: it contains a
  client transmitter address and raw CSI. The in-repo check is a fabricated
  legacy-OFDM record in `wifi-densepose-mtk-bridge/tests/hardware_capture.rs`.
