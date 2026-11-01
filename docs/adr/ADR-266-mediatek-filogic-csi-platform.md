# ADR-266: MediaTek Filogic CSI Platform

- **Status**: accepted
- **Date**: 2026-07-18
- **Deciders**: RuView maintainers
- **Tags**: mediatek, filogic, mt76, csi, openwrt, rust

## Context

RuView needs a high-antenna-count, router-class Wi-Fi sensing path beyond ESP32.
MediaTek Filogic platforms are attractive because the upstream BSD-3-Clause
`mt76` driver supports MT7915/MT792x/MT7996 families and OpenWrt supports
MT7981/MT7986/MT7988 systems. The OpenWrt One (MT7981B + MT7976C) additionally
publishes schematics, platform datasheets, register documentation, serial, and
JTAG access. The BPI-R3 (MT7986 + MT7975N/P) offers dual-band 4x4 radios.

The current upstream `mt76` tree has testmode, debugfs, RX descriptors, and MCU
event plumbing, but no supported public interface for exporting per-packet
complex channel estimates. Public MediaTek SDK material likewise does not expose
an equivalent to Espressif's CSI callback. PHY computation of channel estimates
does not imply that firmware transfers those estimates to host memory.

Existing RuView documents that describe MT7661 CSI-over-UDP or released
MediaTek CSI tools are unverified architectural hypotheses, not supported
hardware claims.

## Decision

1. Use the OpenWrt One as the primary future hardware/upstreaming target and the
   BPI-R3 as the secondary 4x4 validation target.
2. Build a Rust-first simulator and host transport before hardware arrives.
3. Keep the transport independent of private firmware structures. A future
   `mt76` adapter must translate a documented kernel/firmware report into it.
4. Prefer Generic Netlink for capability/control messages and relayfs or a
   bounded character-device stream if sustained CSI volume exceeds Netlink's
   practical throughput.
5. Do not redistribute vendor firmware, private headers, or SDK components.
6. Label simulator frames end-to-end and never present them as physical capture.
7. Do not claim MediaTek hardware CSI support until complex CSI from a physical
   device passes calibration, sequence, timestamp, and repeatability tests.

## Consequences

### Positive

- Development and integration testing can start without fabricating a vendor ABI.
- OpenWrt One provides a repairable, upstream-friendly hardware target.
- The same RuView ingestion path can accept simulator, replay, and future driver data.
- Rust bounds checking isolates untrusted kernel/network input from inference code.

### Negative

- The simulator cannot prove firmware export availability or sensing accuracy.
- A firmware change or MediaTek cooperation may be required before physical CSI exists.
- Router-class builds and driver iteration are slower than MCU firmware development.

### Neutral

- NeuroPilot may later accelerate inference but is unrelated to CSI capture.
- Wi-Fi 7/MLO support remains a later phase after a single-link contract is stable.

## Hardware gates

- Identify a firmware/host report containing complex channel estimates.
- Document dimensions, quantization, chain ordering, subcarrier indexing, lifetime,
  timestamps, sequence behavior, calibration, maximum size, and report rate.
- Validate OpenWrt One first, then BPI-R3 4x4, before considering MT7996/MLO.

## Amendment 1 — vendor CSI on MT7981, still unvalidated

- **Date**: 2026-09-19, recorded here 2026-10-01
- **Status**: proposed. One board model (two units). The hardware gates below are still open.

The 2026-07-18 decision said the public `mt76` tree has no supported CSI export,
and that a firmware change might be required before physical CSI exists. Both
statements need a narrower reading.

Mainline `openwrt/mt76` still has no CSI export. MediaTek's public OpenWrt feed
does: vendor id `0x0ce7`, MCU event `0xc2`, patch
`1001-wifi-mt76-mt7915-csi-implement-csi-support`. On a Wavlink WL-WN586X3
Rev A (MT7981BA + MT7976CN + MT7531AE) that patch, rebased onto the `mt76`
revision pinned by OpenWrt 24.10.8, produced complex samples without a firmware
blob swap. The `0xc2` hook is already in the MT7981 firmware that the OpenWrt
24.10.8 image carries. The
rebased patch and the two userspace fixes are in `firmware/openwrt-wn586x3/`.

What the first capture was: one record, 2.4 GHz, 20 MHz, legacy OFDM, 64
subcarriers, a single chain. CSI arrived only while a client sent frames.
The rate is client traffic, not a sensor sample clock. The capture is not in
this repository.

What it was not:

- Not a calibration, a sequence check, a timestamp check, or a repeatability run.
  The label on the wire is `mediatek:physical-unvalidated`. Plain `mediatek` stays
  reserved for a frame that has passed those gates.
- Not a 4x4 array. Each radio is two chains. The four antennas are a 2.4 GHz pair
  and a 5 GHz pair. Two routers are two receivers. Cross-box phase is not a
  shared local oscillator, so it is not a beam.
- Not the OpenWrt One or the BPI-R3. Those remain the upstream-friendly targets
  in the original decision. The WN586X3 is the board this path was actually run on.

`wifi-densepose-mtk-bridge` translates `mt76-vendor` JSON, or an MtkCSIdump
datagram, into ADR-267 MTC1 and sends it to the sensing server. The server keeps
one snapshot and a bounded ring of per-subcarrier amplitude and phase per
`device_id`. It runs `mediatek-amplitude-heuristic-v0` on mean and peak amplitude,
and `activity-index-v0` on per-subcarrier fluctuation. Both are `CLAIMED`. Neither
is the ESP32 vital-sign path.

One measured property supports further work, and nothing more. Offline, on six
local dump captures from two units (2.4 GHz, HT20, chains rx0 and rx1 of transmit
stream 0), the cross-chain phase `arg(H0·conj(H1))`, summed over subcarriers,
moved 0.014-0.039 rad (p50) between adjacent frames for well-sampled
transmitters. Raw per-chain phase moved about 1.56 rad (p50), which is what
uniformly random phase gives (MEASURED; reproducer
`scripts/mediatek-csi-phase-stability.py`). That shows a stable observable inside
one radio. It does not show that the observable tracks people.

The operator guide is [`docs/mediatek-router-csi.md`](../mediatek-router-csi.md).

## Links

- [ADR-123: BFLD capture path](ADR-123-bfld-capture-path-nexmon-and-esp32.md)
- [ADR-264: RTL8720F radar wire protocol](ADR-264-rtl8720f-radar-wire-protocol.md)
- [ADR-267: MediaTek MIMO CSI wire protocol](ADR-267-mediatek-mimo-csi-wire-protocol.md)
- [MediaTek router CSI guide](../mediatek-router-csi.md)
- [upstream mt76](https://github.com/openwrt/mt76)
- [OpenWrt One](https://openwrt.org/toh/openwrt/one)
- [MediaTek OpenWrt feed](https://git01.mediatek.com/openwrt/feeds/mtk-openwrt-feeds/)
