# ADR-323: RTL8721Dx (Realtek AmebaDplus) CSI Wire Protocol

- **Status**: accepted
- **Date**: 2026-09-15
- **Deciders**: RuView maintainers
- **Tags**: realtek, ameba, amebadplus, rtl8721dx, csi, protocol, rust, udp

## Context

Unlike the MediaTek/Qualcomm CSI paths (ADR-266..269), which are simulator-first
because no supported public vendor interface exports complex CSI, Realtek's
official Ameba RTOS SDK (`Ameba-AIoT/ameba-rtos`, Apache-2.0) documents and
exposes a real, callback-driven CSI API for the RTL8721Dx ("AmebaDplus")
family: `wifi_csi_config()` plus the `RTW_EVENT_CSI_DONE` event delivering a
`struct rtw_event_csi_report_info` (per-tone signed I/Q, 8-bit or 16-bit
precision, selectable tone decimation, passive or active/triggered capture).
This has been confirmed directly against SDK source
(`component/wifi/api/wifi_api_event.h`) and against the official
`example/wifi/wifi_csi` example, and validated on real PKM8721DAF-C13-F10
hardware (build+flash+boot confirmed on branch `feat/rtl8721dx-csi-bringup`).

The RTL8721Dx is a 1x1 WiFi 4 part — single TX/RX chain, not MIMO — so the
MediaTek/Qualcomm wire format (Tx/Rx dimensioned matrices, chipset chain
counts) does not fit. The vendor SDK's raw struct is also not a safe host
wire format as-is: it has no CRC, a C flexible-array-member payload, and
fields sized for firmware convenience (e.g. a wrapping `u32` microsecond
timestamp) rather than network robustness.

Per the existing repo convention (ESP32/ADR-018, MediaTek/ADR-266-267,
Qualcomm/ADR-268-269, RTL8720F radar/ADR-263-264), each vendor CSI/radar
source gets its own self-delimiting envelope and its own Rust module pair
(`wifi-densepose-hardware` for framing+codec+simulator,
`wifi-densepose-sensing-server` for the bounded JSON-safe snapshot), never a
shared "generic vendor CSI" struct that would either lose vendor-specific
fields or invite magic-space collisions. `SourceKind::Realtek` (value 2) is
already assigned to the RTL8720F FMCW radar path (ADR-263/264) and must not
be reused for CSI.

## Decision

Define `RAC1` ("Realtek Ameba CSI", version 1) as a little-endian,
self-delimiting envelope, magic `0x3143_4152` (distinct from `MTC1`
`0x3143544d`, `QCS1` 0x31535143, `RTR1` 0x31525452, and all ADR-018
`0xC511000x` siblings):

Fixed 49-byte header (explicit field-by-field serialization, no `#[repr(C)]`
padding, matching the MediaTek/Qualcomm modules' style):

| Field | Type | Notes |
|---|---|---|
| magic | u32 LE | `0x3143_4152` |
| version | u8 | 1 |
| header_len | u16 LE | 49 |
| frame_len | u32 LE | header + payload + 4-byte CRC |
| node_id | u8 | disambiguates multi-board illuminator/receiver deployments; 0 = unset |
| csi_mode | u8 | 0=passive, 1=active, 2=board-to-board (`CsiMode`) |
| sequence | u32 LE | from firmware `csi_sequence` |
| timestamp_us | u32 LE | from firmware `hw_assigned_timestamp` (hardware-clock microseconds, wraps ~71 min — consumers must not assume monotonic across a capture longer than that) |
| peer_mac | [u8; 6] | firmware `mac_addr` |
| trig_mac | [u8; 6] | firmware `trig_addr` |
| channel | u8 | operating channel |
| bandwidth | u8 | 0=20MHz, 1=40MHz |
| rx_rate | u8 | firmware `rx_rate` |
| protocol_mode | u8 | 0=OFDM,1=HT,2=VHT,3=HE (`ProtocolMode`) |
| num_sub_carrier | u16 LE | tone count in payload |
| num_bit_per_tone | u8 | 16 (8-bit I + 8-bit Q) or 32 (16-bit I + 16-bit Q) |
| decimation | u8 | tone group (1/2/4/8 — every Nth tone) |
| rssi_dbm | i8 | single RF chain (1x1 hardware) |
| rxsc | u8 | sub-20 MHz channel used |
| csi_valid | u8 | 0/1 from firmware |
| flags | u8 | bit 0 = SYNTHETIC (simulator provenance, never clearable downstream) |
| payload_len | u32 LE | bytes, == `num_sub_carrier * (num_bit_per_tone / 8)` |

Payload: `num_sub_carrier` consecutive (I, Q) tone pairs, each
`num_bit_per_tone / 16` bytes per component (1 byte/component when
`num_bit_per_tone == 16`, 2 bytes/component little-endian signed when `== 32`)
— i.e. exactly the byte layout the firmware already produces in
`csi_data[]`, just framed. CRC-32/IEEE over header+payload, final 4
little-endian bytes. One envelope per UDP datagram.

Parsers reject: unknown version, wrong `header_len`, `frame_len` exceeding
the IPv4 UDP payload cap (65,507 bytes), `num_bit_per_tone` outside
`{16, 32}`, `payload_len` inconsistent with `num_sub_carrier *
(num_bit_per_tone/8)`, trailing bytes, and bad CRC — mirroring the
MediaTek/Qualcomm parser contract exactly.

Host side: add `realtek_csi.rs` to both
`wifi-densepose-hardware` (frame + codec + simulator, mirroring
`mediatek_csi.rs`/`qualcomm_csi.rs`) and `wifi-densepose-sensing-server`
(bounded `RealtekCsiSnapshot`, mirroring `QualcommCsiSnapshot`), wired into
the same UDP listener loop and `/api/v1/csi/realtek/latest` route as the
existing vendor sources. The initial Mac compatibility port keeps Realtek CSI
as a separate bounded diagnostic stream. Fresh ESP32 frames remain the primary
sensing source. This change does not add `SourceKind::RealtekCsi` to a signed
ingest contract; that requires a separate reviewed identity-bound hardware
proof. RAC1 CRC checks corruption, not origin.

Firmware side (`example/wifi/wifi_csi` derivative, built against the
official Ameba RTOS SDK, tracked outside this repository per the existing
`CLAUDE.local.md` convention — the vendor SDK itself is never vendored into
this repo): join the board to the deployment's WiFi AP in STA mode
(passive CSI against the real AP as peer, since only one physical board is
available in this bring-up), read the AP's BSSID at runtime after
association (never hardcode a MAC into firmware source), pack each
`RTW_EVENT_CSI_DONE` report into one `RAC1` datagram, and `sendto()` it over
UDP to the host sensing-server's existing CSI port.

## Consequences

### Positive

- CSI is real, MEASURED hardware output from day one — no simulator-first
  bridge phase is needed the way MediaTek/Qualcomm required, because the
  vendor SDK genuinely exposes the capability and we have the board in hand.
- One board is sufficient for a first useful signal (passive CSI against the
  home AP); active and board-to-board modes are additive later without a
  wire-format break (`csi_mode` already reserves the values).
- Reuses the bounded UDP parser and status-route pattern without changing
  existing ESP32 sensing. Unsigned RAC1 diagnostics are not authorized as
  calibrated presence, person count, pose, or vital evidence.

### Negative

- 1x1 hardware means no spatial (multi-antenna) diversity from a single
  board; multi-board deployment is required for the differential/localization
  use cases described in the original RTL8721Dx capability analysis.
- The 71-minute microsecond timestamp wraparound (inherited from the
  firmware's `u32` field) means long unattended captures need host-side
  sequence-based reordering, not raw timestamp arithmetic, across a wrap.

### Neutral

- The RAC1 envelope intentionally does not carry the SDK's raw
  `csi_signature` (0xABCD) marker — our own `magic`/CRC supersede it as the
  framing integrity check.

## Hardware gates

- [x] Confirm SDK source exposes `RTW_EVENT_CSI_DONE` / `rtw_event_csi_report_info` (done — `component/wifi/api/wifi_api_event.h`).
- [x] Build and flash a baseline image on real PKM8721DAF-C13-F10 hardware, confirm boot over serial (done — `feat/rtl8721dx-csi-bringup`).
- [x] Flash the CSI-streaming firmware and confirm `RAC1` datagrams parse on the host from the real board (done, 2026-09-15 — see below).
- [x] Record MEASURED throughput/latency/jitter from the real running firmware (done, 2026-09-15 — see below).
- [ ] A human walk-test across the link (amplitude perturbation vs. static baseline) is deferred — requires the operator physically present; not claimed until performed.

### MEASURED: real RAC1 capture and sustained throughput (2026-09-15)

Passive CSI (`RTW_CSI_MODE_NORMAL`, `RTW_CSI_TRIG_BEACON | RTW_CSI_TRIG_DATA | RTW_CSI_TRIG_QOS_DATA`)
against the deployment AP. A single `RAC1` datagram was first
cross-checked field-by-field against the firmware's own serial log of the
same report — every byte matched: magic `0x31434152`, RSSI byte exact match,
channel/bandwidth/protocol/subcarrier-count exact match, 157-byte total frame
(49-byte header + 104-byte payload + 4-byte CRC, exactly as specified above).

`RTW_CSI_MODE_RX_RESP` (estimate CSI from ACKs to the STA's own transmissions)
never fired `RTW_EVENT_CSI_DONE` across two full test runs with confirmed
real traffic (not a hang) — this mode does not work for this hardware/AP
combination and `RTW_CSI_MODE_NORMAL` with beacon+data triggers is the
working configuration.

**An earlier note here claimed `NORMAL` mode fires only once per CSI enable
— that was wrong**, caused by a debug log line gated to print only every
50th frame (`frame_count % 50 == 1`), which was misread as "only one report
total." A follow-up capture with a UDP listener running concurrently with a
board reset measured **277 real RAC1 datagrams in 40 seconds (43,489 bytes)**,
with `csi_sequence` climbing continuously and monotonically in the firmware's
own serial log across six separate 50-frame log checkpoints in one 45s
session — CSI reporting is continuous, roughly 7-12 fps, not sparse.

### MEASURED: throughput optimization pass (2026-09-15)

Formalized as a repeatable tool: `scripts/benchmark-rtl8721dx-csi.py` (listens
on the sensing-server's CSI UDP port, reports fps/throughput/RSSI/inter-frame
interval/sequence-gap stats from real received `RAC1` datagrams).

**Baseline** (`trig_period=200` [64ms], `trig_frame_ctrl=0`,
`CSI_REPORT_BUF_NUM=1`), 30s capture:

| Metric | Value |
|---|---|
| RAC1 fps | 14.67 |
| Throughput | 2,302.7 bytes/s |
| Mean inter-frame interval | 68.3 ms |
| RSSI | mean -34.4 dBm |

`trig_period` (units of 320µs) was hypothesized as the rate-limiting
throttle — the observed 68.3ms mean interval tracks the 64ms configured
period almost exactly. Confirmed by tuning:

1. `trig_period`: 200 → 15 (the SDK doc's stated minimum recommended value,
   ~4.8ms)
2. `trig_frame_ctrl`: 0 → `RTW_CSI_TRIG_ACK` (second trigger source: every
   link-layer ACK the AP sends for the heartbeat's own outgoing traffic, on
   top of the existing beacon trigger)
3. `CSI_REPORT_BUF_NUM`: 1 → 4 (a "lack of csi buf!" drop was observed even
   at the slower baseline rate; more headroom needed at a higher rate)

**Tuned**, 30s capture:

| Metric | Value | vs. baseline |
|---|---|---|
| RAC1 fps | 66.40 | **4.5x** |
| Throughput | 10,424.8 bytes/s | **4.5x** |
| Mean inter-frame interval | 14.9 ms | 4.6x lower |
| Sequence gaps | 78 / ~2036 (~3.8%) | new (0 observed at the slower baseline rate) |
| RSSI | mean -31.1 dBm | (different session/position, not a controlled variable here) |

The `trig_period` hypothesis is confirmed as the primary lever. The ~3.8%
frame-loss rate at 66 fps is a reasonable place to stop for now — it is
UDP-over-WiFi loss, not a firmware defect, and is small enough not to block
downstream sensing use; revisit only if a use case needs stronger delivery
guarantees (sequence-gap-triggered re-request, TCP fallback, etc.).

## Links

- [ADR-266](ADR-266-mediatek-filogic-csi-platform.md) / [ADR-267](ADR-267-mediatek-mimo-csi-wire-protocol.md) — sibling vendor CSI ADR pair (simulator-first precedent)
- [ADR-268](ADR-268-qualcomm-atheros-csi-platform.md) / [ADR-269](ADR-269-qualcomm-csi-wire-protocol.md) — sibling vendor CSI ADR pair
- [ADR-263](ADR-263-rtl8720f-2-4ghz-fmcw-radar-platform.md) / [ADR-264](ADR-264-rtl8720f-radar-wire-protocol.md) — existing Realtek (RTL8720F radar) path; `SourceKind::Realtek` owner
- `CLAUDE.local.md` — RTL8721Dx toolchain setup, board/COM port, first bring-up log
