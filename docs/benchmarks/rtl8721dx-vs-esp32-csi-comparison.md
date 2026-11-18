# RTL8721Dx vs. the ESP32 Family (S3 / C6 / C5): WiFi CSI Sensing Comparison

The radio that captures Channel State Information (CSI) determines how fast, how
private, and how reliably RuView can sense a room. Four WiFi chips are relevant
to that role today. This document compares them against each other and states,
claim by claim, which numbers were measured on real hardware and which were
only designed, configured, or never tested.

It answers three questions: which chip is the current production node, which
measured numbers exist for each chip, and which gaps block a "which is faster"
conclusion.

## Contents

- [What does this comparison decide?](#what-does-this-comparison-decide)
- [How are the claims tagged?](#how-are-the-claims-tagged)
- [Which hardware is in scope?](#which-hardware-is-in-scope)
- [How does each chip capture CSI?](#how-does-each-chip-capture-csi)
- [How do the wire formats compare?](#how-do-the-wire-formats-compare)
- [What has actually been measured?](#what-has-actually-been-measured)
- [How mature is each firmware?](#how-mature-is-each-firmware)
- [What are the known limitations?](#what-are-the-known-limitations)
- [Which chip should you pick?](#which-chip-should-you-pick)
- [What is still missing?](#what-is-still-missing)
- [References](#references)
- [See also](#see-also)

## What does this comparison decide?

- **None of the four chips has an antenna-count advantage.** All four are 1×1
  single-antenna radios. Better spatial coverage comes from deploying more
  boards around a space, not from choosing a different chip.
- **The RTL8721Dx can listen without injecting traffic of its own.** The ESP32
  family always transmits extra WiFi traffic to produce a CSI reading; the
  RTL8721Dx can observe a router's existing traffic. That matters for a sensor
  that should be quieter on the network.
- **The two ESP32 chips that were built and tested this way are more mature as
  products.** The ESP32-C6 has measured multi-board timing numbers; the
  ESP32-S3 has the OTA, on-device DSP, WASM and CI stack.
- **"RTL8721Dx is faster" is an artefact of only one chip being benchmarked.**
  A repeatable real-hardware CSI-rate benchmark exists for the RTL8721Dx only.
  The ESP32-S3 and ESP32-C6 have never been run through it, so their published
  rates are configured values, not results.
- **The ESP32-C5 is the most capable radio on paper and the least tested.** It
  is the only candidate that can see both the 2.4 GHz and 5 GHz bands, and
  nothing has confirmed that its chip exposes the CSI data RuView needs.
- **Multi-board throughput on the RTL8721Dx degrades under contention**, and no
  mitigation is implemented. The ESP32-C6 has already shipped and measured the
  analogous solution.

## How are the claims tagged?

| Tag | Meaning |
|---|---|
| **MEASURED** | Reproduced from real hardware, with a witness artifact or a recorded bench run in this repository. |
| **DESIGN** | A configured or intended value, not verified against real hardware. |
| **UNKNOWN** | Genuinely undocumented. Stated as a gap, not guessed. |

Anything about the ESP32-C5 is **UNKNOWN** or public datasheet only: no RuView
code exists for it and nothing has been measured.

One rule applies throughout. A number measured on a public benchmark is not
the same kind of evidence as a number measured on live ESP32 hardware in a
room, and the two are never substituted for each other.

## Which hardware is in scope?

| | ESP32-S3 | ESP32-C6 | ESP32-C5 | RTL8721Dx (AmebaDplus) |
|---|---|---|---|---|
| Module | ESP32-S3-DevKitC-1 | ESP32-C6-DevKitC-1 | ESP32-C5-WROOM-1 (public spec only) | PKM8721DAF-C13-F10 |
| CPU | Dual-core Xtensa LX7 @ 240 MHz | Single-core RISC-V @ 160 MHz + LP-RISC-V core | Single-core RISC-V @ 240 MHz | Dual-core: KM4 (Cortex-M33, 240 MHz) + KM0 (96 MHz) |
| RAM / Flash | 8 MB PSRAM / 8 MB flash | 320 KB SRAM, no PSRAM / up to 8 MB flash | 384 KB SRAM + external PSRAM support / not fixed | 512 KB + 160 KB SRAM / 4 MB flash |
| WiFi | 1×1 SISO, 802.11n, 20/40 MHz, 2.4 GHz only | 1×1 SISO, 802.11ax (WiFi 6), 20 MHz, 2.4 GHz only | 1×1, 802.11ax (WiFi 6), dual-band 2.4 + 5 GHz (public spec) | 1×1 SISO, 802.11a/b/g/n (WiFi 4), 2.4 + 5 GHz; 20 MHz in the tested configuration |
| Extra radios | — | 802.15.4 (Thread/Zigbee) | Zigbee 3.0, Thread 1.4 (public spec) | BLE 5.0 |
| Power | 150–200 mA active, 10 µA deep sleep (**MEASURED**, [ADR-028](../adr/ADR-028-esp32-capability-audit.md)) | ~10 µA standby via GPIO-wake deep sleep (**MEASURED**, [ADR-110](../adr/ADR-110-esp32-c6-firmware-extension.md) §2.6); ~5 µA LP-core hibernation path built but not current-measured (**DESIGN**) | Not tested (**UNKNOWN**) | Not measured (**UNKNOWN**) |
| Unit cost | ~$8–12 per node, ~$54 for a 3-node kit (**DESIGN**, [ADR-028](../adr/ADR-028-esp32-capability-audit.md)) | Not documented in-repo (**UNKNOWN**) | Not documented in-repo (**UNKNOWN**) | Not documented in-repo or in public pricing (**UNKNOWN**) |

The ESP32-C6 has no PSRAM, which caps how much of the ESP32-S3 on-device DSP and
WASM stack can run on it unmodified.

## How does each chip capture CSI?

| | ESP32-S3 | ESP32-C6 | ESP32-C5 | RTL8721Dx |
|---|---|---|---|---|
| API | `esp_wifi_set_csi_config()` + `esp_wifi_set_csi_rx_cb()` + `esp_wifi_set_csi(true)` | Same ESP-IDF CSI API, extended with HE-LTF frame tagging | Unconfirmed — Espressif's own C5 material does not document CSI support explicitly (**UNKNOWN**) | `wifi_csi_config()` with `RTW_CSI_ACT_CFG` / `RTW_CSI_ACT_EN` |
| Trigger model | Always-on hybrid: promiscuous MGMT-frame capture plus self-injected probe/NDP frames, so a reading arrives regardless of ambient traffic. No pure-passive mode | Same hybrid model as the S3, plus optional TWT (Target Wake Time): the sensor negotiates a scheduled wake slot with a cooperative AP, making CSI cadence deterministic rather than opportunistic ([ADR-110](../adr/ADR-110-esp32-c6-firmware-extension.md) §2.5) | Not built or tested (**UNKNOWN**) | Explicit per-frame-type trigger bitmask (`trig_frame_mgnt` / `trig_frame_data` / `trig_frame_ctrl`). Genuinely passive (`RTW_CSI_MODE_NORMAL`) is a distinct mode from triggered capture |
| Precision control | Fixed: `lltf_en` / `htltf_en` / `stbc_htltf2_en` booleans, with no I/Q bit-depth or decimation knob | Same as the S3, plus PPDU-type tagging (HT / HE-SU / HE-MU / HE-TB) written into the wire format | Unconfirmed (**UNKNOWN**) | `RTW_CSI_ACCU_1BYTE` / `_2BYTES` (8/16-bit I/Q), `RTW_CSI_GROUP_NUM` decimation (every 1st/2nd/4th/8th tone), tunable `trig_period` |
| Subcarriers | 64 (802.11n, HT-LTF) | 64 in practice under ESP-IDF v5.4 (HT fallback); 242 subcarriers confirmed on ESP-IDF ≥ 5.5 with a real HE link — v5.4 silently downconverts even on a confirmed-HE link ([ADR-110](../adr/ADR-110-esp32-c6-firmware-extension.md) §1, hardware-confirmed 2026-06-11, issue [#1005](https://github.com/ruvnet/RuView/issues/1005)) | Unconfirmed (**UNKNOWN**) | 52 (802.11n, 20 MHz configuration) |

Two findings here are worth isolating.

The RTL8721Dx's passive trigger mode and the ESP32-C6's WiFi 6 HE-LTF and TWT
capabilities are the two non-obvious capabilities in this table. Both carry
caveats: the RTL8721Dx's passive trigger bitmask silently captures nothing if
left at its default (`0`), and its `RTW_CSI_MODE_RX_RESP` mode never fired
`RTW_EVENT_CSI_DONE` across two full test runs with confirmed real traffic
(**MEASURED**, [ADR-323](../adr/ADR-323-rtl8721dx-ameba-csi-wire-protocol.md)).
The ESP32-C6's HE-LTF result requires ESP-IDF ≥ 5.5 and a WiFi 6 access point,
which most home routers do not provide.

## How do the wire formats compare?

| | ESP32 (ADR-018, shared by S3 and C6) | RTL8721Dx (ADR-323, RAC1) |
|---|---|---|
| Magic | `0xC5110001`, plus five sibling packet types on the same port, including a separate `0xC511A110` C6 mesh time-sync packet | `0x3143_4152` |
| Header size | 20 bytes; the C6 repurposes two previously reserved bytes for PPDU type and bandwidth/sync flags ([ADR-110](../adr/ADR-110-esp32-c6-firmware-extension.md) §2.3) | 49 bytes |
| Antenna field | `n_antennas` (u8); the parser supports up to 4, although every real chip here is 1×1, so this is forward-looking headroom rather than a capability in use | No antenna field — the format does not claim support for more than the hardware has |
| Per-tone precision | Fixed I/Q width, not configurable in the wire format. The C6's HE-LTF mode changes the subcarrier count per frame, which downstream consumers must handle as non-fixed ([ADR-110](../adr/ADR-110-esp32-c6-firmware-extension.md) §3.2) | `num_bit_per_tone` field (16 or 32), matching the hardware's configured precision per frame |
| Integrity | None documented in the 20-byte header | CRC-32/IEEE trailer |
| Multi-node ID | `node_id` (u8) | `node_id` (u8) — the same pattern; both support fleet disambiguation |
| Cross-node time sync | C6 only: a dedicated 32-byte sync packet (`SyncPacket`, magic `0xC511A110`) carrying node id, local and epoch timestamps, and sequence, with shipped Rust (10 tests) and Python (6 tests) decoders wired into the sensing server ([ADR-110](../adr/ADR-110-esp32-c6-firmware-extension.md) §4.3) | Not built |

Both base wire formats are hand-rolled and manually serialised, with no external
schema or protobuf, and both live in the `wifi-densepose-hardware` crate
convention (`esp32_parser.rs` / `realtek_csi.rs`), so basic host-side
integration effort is comparable. The C6's mesh time-sync layer is a materially
larger, already-shipped capability that neither the S3 nor the RTL8721Dx has an
equivalent of.

## What has actually been measured?

This is where the four chips separate. The measurements do not line up with the
headline "which is faster" framing, because each chip was measured against a
different question.

### ESP32-C6 — MEASURED (bench run, [ADR-110](../adr/ADR-110-esp32-c6-firmware-extension.md) §4.2, [WITNESS-LOG-110](../WITNESS-LOG-110.md))

| Metric | Measured | Target |
|---|---|---|
| Boot time | 353 ms | — |
| Time-sync init | 413 ms | — |
| Firmware image / flash slack | 1003 KB / 46 % | — |
| Free heap | 310 KiB | — |
| CSI callbacks | firing at 64 subcarriers/frame on channel 1 under background traffic | — |
| Cross-board ESP-NOW RX rate (5-minute soak) | 99.56 % (2689 / 2701 beacons) | — |
| Cross-board TX failures (5-minute soak) | 0 on either board | — |
| Raw cross-board offset stdev | 411.5 µs | — |
| **EMA-smoothed offset stdev** | **104.1 µs** | ≤ 100 µs (§2.4) |
| Range reduction (smoothed vs raw) | 4.70× peak-to-peak | — |
| Measured crystal skew between bench boards | 1.4 ppm | ESP32 spec ±10 ppm |
| Raw 802.15.4 RX | 0 frames over 60 s, 240 s and 300 s soaks | — |

The 802.15.4 row is the load-bearing caveat. The primary working mesh transport
turned out to be ESP-NOW rather than 802.15.4, and the 802.15.4 receive path
did not work under ESP-IDF v5.4. **No CSI-frames-per-second throughput
benchmark exists for the C6 either**, so its numbers and the RTL8721Dx's are
not the same metric.

### ESP32-S3 — not measured for CSI throughput

The README states "~20 Hz" and [ADR-028](../adr/ADR-028-esp32-capability-audit.md)
states "20-100 Hz (configurable)". Both are **DESIGN** values, not benchmark
results. The same audit explicitly flags a separate "54,000 fps" figure
elsewhere in the documentation as a benchmark claim not measured at audit time,
requiring Criterion benchmarks on target hardware.

### RTL8721Dx — MEASURED (2026-09-15, [ADR-323](../adr/ADR-323-rtl8721dx-ameba-csi-wire-protocol.md))

A single `RAC1` datagram was cross-checked field-by-field against the
firmware's own serial log of the same report: every byte matched — magic
`0x31434152`, byte-exact RSSI, exact channel/bandwidth/protocol/subcarrier
count, 157-byte total frame (49-byte header + 104-byte payload + 4-byte CRC).

Passive capture against the deployment AP, with a concurrent UDP listener and a
board reset, measured **277 real RAC1 datagrams in 40 seconds (43,489 bytes)**
with `csi_sequence` climbing continuously — CSI reporting is continuous at
roughly 7–12 fps, not sparse.

A repeatable benchmark tool,
[`scripts/benchmark-rtl8721dx-csi.py`](../../scripts/benchmark-rtl8721dx-csi.py),
then produced the throughput figures over 30-second captures:

| Metric | Baseline (`trig_period=200`) | Tuned | Change |
|---|---|---|---|
| RAC1 fps | 14.67 | **66.40** | 4.5× |
| Throughput | 2,302.7 bytes/s | **10,424.8 bytes/s** | 4.5× |
| Mean inter-frame interval | 68.3 ms | 14.9 ms | 4.6× lower |
| Sequence gaps | 0 observed | 78 / ~2036 (~3.8 %) | new |
| RSSI | mean −34.4 dBm | mean −31.1 dBm | different session, not a controlled variable |

The tuning lever was `trig_period` (200 → 15), with `trig_frame_ctrl` extended
with `RTW_CSI_TRIG_ACK` and `CSI_REPORT_BUF_NUM` raised from 1 to 4 after a
"lack of csi buf!" drop was observed even at the slower baseline rate. The
~3.8 % frame-loss rate at 66 fps is UDP-over-WiFi loss, not a firmware defect.

### ESP32-C5 — no data of any kind

No RuView code exists for it and nothing has ever been measured.

### The honest reading

The RTL8721Dx is the only chip with a **MEASURED** CSI-frames-per-second
number. The ESP32-C6 is the only chip with a **MEASURED** multi-board
timing-accuracy number. Neither measurement answers the other chip's question.
"RTL8721Dx is the fastest CSI source" is not a claim this data supports — it is
the only CSI-rate number that currently exists.

One figure from the 2026-09-15 comparison session has no in-repo witness
artifact: per-node throughput degradation on the RTL8721Dx when several boards
share a channel. It is recorded here as reported, not as **MEASURED**, because
[ADR-323](../adr/ADR-323-rtl8721dx-ameba-csi-wire-protocol.md) does not carry
it. Closing that gap is listed under [What is still missing](#what-is-still-missing).

## How mature is each firmware?

The ESP32-S3 and ESP32-C6 are far more mature as products than the RTL8721Dx:

- OTA firmware update over HTTP (S3; the C6 shares the same base firmware project).
- A 4-tier on-device DSP pipeline on the S3: raw CSI → phase, Welford-variance
  and top-K feature extraction → vitals, presence and fall detection → WASM-based
  programmable sensing with Ed25519-signed modules.
- Runtime configuration via NVS, without reflashing.
- Multi-node coordination: TDM scheduling on the S3
  ([ADR-029](../adr/ADR-029-ruvsense-multistatic-sensing-mode.md)); the C6 goes
  further with a shipped, sub-100 µs cross-board time-sync layer and real
  host-side decoders.
- QEMU-based CI with fuzz testing and a 14-configuration NVS test matrix, plus
  a binary-size CI gate.
- C6-only: TWT deterministic wake scheduling, an 802.15.4 side-channel radio
  (the time-sync fallback path, whose receive path did not work under ESP-IDF
  v5.4), and an LP-core hibernation path for battery-powered seed nodes.

The RTL8721Dx is currently a raw CSI streamer: no OTA, no on-device DSP, no
WASM, no CI, no fall detection, no NVS-equivalent runtime configuration and no
cross-node time sync. The ESP32-C5 has none of this either, for the opposite
reason — nobody has started building it.

## What are the known limitations?

**ESP32-S3 and ESP32-C6.** `wifi-densepose-hardware/src/lib.rs` frames the
whole CSI-over-commodity-WiFi approach as provisional
([ADR-153](../adr/ADR-153-ieee-802-11bf-sensing-protocol-layer.md)): no
commodity silicon implements IEEE 802.11bf-2025 yet, so the
`OpportunisticCsiBridge` maps today's ESP32 CSI extraction onto the
standardised report path until an OTA binding exists. Presence detection is a
variance-threshold heuristic with false positives on RF interference; person
count is a slot-capacity heuristic, not a learned classifier; no pose model
ships pretrained. C6-specific: its HE-LTF result requires ESP-IDF ≥ 5.5 and a
real 11ax access point; its 802.15.4 time-sync radio does not receive frames
under ESP-IDF v5.4, worked around via ESP-NOW rather than fixed at the driver
level; and it has no PSRAM.

**ESP32-C5.** Entirely unknown. Its CSI support is unconfirmed in Espressif's
own public material, let alone tested by RuView. Every "on paper" advantage
above is a reason to investigate, not a reason to deploy.

**RTL8721Dx.** Single-antenna hardware means no spatial diversity from a
single board. The 71-minute microsecond timestamp wraparound inherited from the
firmware's `u32` field means long unattended captures need host-side
sequence-based reordering rather than raw timestamp arithmetic across a wrap.
`RTW_CSI_MODE_RX_RESP` does not work on this hardware and access point
combination. The human walk-test gate — amplitude perturbation against a static
baseline — is **deferred** in
[ADR-323](../adr/ADR-323-rtl8721dx-ameba-csi-wire-protocol.md) and is not
claimed: it requires an operator physically present. Unsigned RAC1 diagnostics
are not authorised as calibrated presence, person count, pose or vital
evidence.

**A defect found during this integration and since fixed.** The comparison
session found that the host server's `--source auto` mode silently failed to
bind the UDP CSI receiver on any machine with its own WiFi adapter,
independently of vendor. That was fixed by
[ADR-025](../adr/ADR-025-macos-corewlan-wifi-sensing.md) Amendment 1
(2026-10-01), which made `--source auto` bind the UDP receiver when it
resolves to host WiFi, with the measured result `--source auto` on a quiet
port → `wifi` with `udp_receiver=true`. The finding is recorded here because
it applied to every CSI vendor, not only the Realtek path.

**One withdrawn claim.** An earlier comparison held that the C6 firmware image
is 9 % smaller than the S3's. That comparison was apples-to-oranges, because the
S3 default build includes display and WASM while the C6 build does not. The
like-for-like measurement is in [WITNESS-LOG-110](../WITNESS-LOG-110.md) §B5:
S3 production 1109 KB (47 % slack), S3 without display and WASM 886 KB (53 %
slack), C6 1003 KB (46 % slack). The honest reading is that the C6 binary is
117 KB, or 13 %, **larger** than an equivalent S3 build, because it carries the
802.15.4 PHY and OpenThread MTD stack. The original 9 % claim is withdrawn.

## Which chip should you pick?

There is no single winner, because the four chips answer different questions.

- **ESP32-S3** for a production-leaning deployment today: the most mature
  OTA, DSP, WASM and CI stack, the most prior investment, the lowest risk.
- **ESP32-C6** when several boards must work together with tight timing, or
  when WiFi 6's richer CSI resolution is wanted against a compatible router.
  It is the only chip here with a **MEASURED** multi-board coordination result.
- **RTL8721Dx** where genuinely passive sensing matters, or where per-tone
  precision and decimation tuning is needed — capabilities the fixed ESP32 CSI
  configuration does not offer. It is the least mature product and the most
  configurable radio, and the only one with a **MEASURED** CSI frame-rate
  number.
- **ESP32-C5** is not a deployment choice yet — not because it is weak, but
  because nobody has confirmed it can do the job at all.

## What is still missing?

Three gaps block a "which is fastest" conclusion, and each is cheap to close:

1. **Run the existing RTL8721Dx benchmark against ESP32-S3 and ESP32-C6
   hardware.** The tool is already repeatable
   ([`scripts/benchmark-rtl8721dx-csi.py`](../../scripts/benchmark-rtl8721dx-csi.py));
   only the ESP32 targets are missing.
2. **Record the multi-board contention measurement for the RTL8721Dx** in
   [ADR-323](../adr/ADR-323-rtl8721dx-ameba-csi-wire-protocol.md), with the raw
   capture, so the reported degradation becomes auditable rather than
   anecdotal.
3. **Perform the deferred human walk-test on the RTL8721Dx** — amplitude
   perturbation against a static baseline — to close the one hardware gate
   still open in that ADR.

Until those exist, every "which is fastest" claim rests on incomplete,
mismatched measurements. The remedy is more real benchmarks, not more argument
from datasheets.

## References

- [ADR-018 — ESP32 dev implementation](../adr/ADR-018-esp32-dev-implementation.md) — ESP32 binary frame format.
- [ADR-025 — macOS CoreWLAN WiFi sensing](../adr/ADR-025-macos-corewlan-wifi-sensing.md) — Amendment 1, the `--source auto` UDP receiver bind fix.
- [ADR-028 — ESP32 capability audit](../adr/ADR-028-esp32-capability-audit.md) — ESP32-S3 hardware and capability record, including the audit of the 54,000 fps claim.
- [ADR-029 — RuVSense multistatic sensing mode](../adr/ADR-029-ruvsense-multistatic-sensing-mode.md) — S3 multi-node TDM scheduling.
- [ADR-110 — ESP32-C6 firmware extension](../adr/ADR-110-esp32-c6-firmware-extension.md) — HE-LTF, 802.15.4, TWT, LP-core hibernation, and the P10 measured numbers.
- [ADR-153 — IEEE 802.11bf sensing protocol layer](../adr/ADR-153-ieee-802-11bf-sensing-protocol-layer.md) — why the ESP32 CSI path is provisional.
- [ADR-266 / ADR-267 — MediaTek CSI platform and wire protocol](../adr/ADR-266-mediatek-filogic-csi-platform.md) — sibling vendor CSI pair.
- [ADR-268 / ADR-269 — Qualcomm CSI platform and wire protocol](../adr/ADR-268-qualcomm-atheros-csi-platform.md) — sibling vendor CSI pair.
- [ADR-263 / ADR-264 — RTL8720F FMCW radar platform and wire protocol](../adr/ADR-263-rtl8720f-2-4ghz-fmcw-radar-platform.md) — existing Realtek path.
- [ADR-323 — RTL8721Dx (Realtek AmebaDplus) CSI wire protocol](../adr/ADR-323-rtl8721dx-ameba-csi-wire-protocol.md) — the RAC1 wire format, the measured throughput, and the open hardware gates.
- [WITNESS-LOG-110](../WITNESS-LOG-110.md) — ESP32-C6 witness records, including the §B5 binary-size comparison.
- [`scripts/benchmark-rtl8721dx-csi.py`](../../scripts/benchmark-rtl8721dx-csi.py) — the repeatable CSI-rate benchmark.
- [`v2/crates/wifi-densepose-hardware/src/realtek_csi.rs`](../../v2/crates/wifi-densepose-hardware/src/realtek_csi.rs) — the host-side RAC1 parser.

## See also

- [Proof of capabilities](../proof-of-capabilities.md) — which sensing claims are
  validated, and at what level.
- [Evidence matrix](../evidence-matrix.md) — the per-claim validation status of
  the README's headline statements.
- [WiFi pose efficiency frontier](wifi-pose-efficiency-frontier.md) — how the
  sensing models compare on accuracy against published alternatives.
- [MM-Fi WiFi sensing study](mmfi-wifi-sensing-study.md) — the public-benchmark
  evidence for the pose and presence models.
