# Local C5 L-LTF8 producer format claim v1

This is a local experimental format contract, not an official firmware release,
hardware attestation, calibrated phase or sensing-accuracy result. The previous
C5 profile zero-initialized `lltf_bit_mode`; in ESP-IDF 6.0.2 that selects 12-bit
components, not the signed int8 packing assumed by ADR-018. Its observed
106-byte callbacks therefore do not establish 53 native complex int8 samples.

Primary references: [ESP-IDF v6.0.2 CSI configuration](https://github.com/espressif/esp-idf/blob/v6.0.2/components/esp_wifi/include/esp_wifi_he_types.h),
[C5 vendor features](https://github.com/espressif/esp-idf/blob/v6.0.2/docs/en/api-guides/wifi-driver/wifi-vendor-features.rst),
and [Espressif's packing clarification](https://github.com/espressif/esp-idf/issues/18982#issuecomment-5420118269).

The new opt-in profile requests only legacy/forced L-LTF with
`lltf_bit_mode=1`, reads the actual configuration through the pinned SDK MAC+PHY adapter described below, and
withholds CSI if that readback differs. A callback can carry this claim only
when its original metadata reports non-HT OFDM (`cur_bb_format=1`, 11g/11a),
no secondary channel, valid channel estimate and exactly 106
bytes. Rejected metadata is logged at a bounded rate without raw samples.
The profile sets STA bandwidth to `WIFI_BW20` before association and verifies
it through `esp_wifi_get_bandwidth` before CSI acquisition. C5 has no per-frame
`cwb` field; the suffix explicitly reports that interface readback, combined
with the original non-HT OFDM/secondary-none callback metadata, not an invented
per-frame bandwidth value. CSI format and the opaque SDK `val_scale_cfg=0` write footprint are checked;
HE-STBC selection is irrelevant to this non-HT profile.
Physical readback/callback validation is required before managed capture;
unknown shape, estimator-validity starvation or another PHY remains blocked.

After the normal 20-byte ADR-018 header and declared 106-byte IQ payload, a
16-byte suffix carries the following exact fields. It uses no existing flag
bit, avoiding the HE-invalid/first-word bit-5 collision. The legacy live parser
can still read the declared IQ payload; the suffix must never enter edge DSP.

| Bytes | Meaning |
|---|---|
| 0–3 | ASCII `C5L8` |
| 4 | Contract version 1 |
| 5 | Component width 8 bits |
| 6 | SDK-documented signed int8 imaginary/real order, ID 1 |
| 7 | SDK L-LTF53 order `[0..26, -26..-1]`, ID 1 |
| 8 | SDK configuration state matched through the pinned MAC+PHY adapter, 1 |
| 9 | Original `cur_bb_format`, required 1 |
| 10 | Original secondary-channel field, required 0 |
| 11 | STA bandwidth API readback in MHz, required 20 |
| 12 | Original `rx_channel_estimate_info_vld`, required 1 |
| 13 | Original `first_word_invalid`, 0 or 1 |
| 14–15 | Original IQ byte count, little-endian 106 |

Native sample slots stay in the documented order, including invalid DC slot
0. If the SDK marks its first word invalid, the existing firmware sanitizer
zeros four bytes and marks that fact; in this explicit 8-bit format those are
two complex positions, not a renumbered/deleted prefix. The host must retain
the actual received bytes and validity mask. Any magnitude feature projection
must have its own versioned identity and must not claim that 53 slots form a
uniform/padded 64-point FFT or that interpolation adds measured tones. The
documented component convention does not qualify phase; no learner may
consume phase on the strength of this format claim alone.

## Pinned C5 readback adapter (local experimental implementation)

Physical Node23 diagnostics showed that ESP-IDF v6.0.2's successful public
`esp_wifi_get_csi_config` call populated only structure bits0–11. The caller's
zero-filled bits12–31 were not measurements: the getter does not return
`val_scale_cfg`, `dump_ack_en` or `lltf_bit_mode`. The adapter never seeds those
fields from the request and never treats their zeroes as readback.

The exact linked setter still consumes the full four-byte configuration:
`ic_csi_set_config` calls the **normal** `phy_chan_dump_cfg` directly, then
`phy_lltf_mask_en`. The `_752` alternative is discarded from this image.
The public getter checks the supported MAC fields; a separate read-only
snapshot checks the vendor setter's control-register write footprint:

| Control address | Required mask | Expected masked value | Bounded interpretation |
| --- | --- | --- | --- |
| `0x600a790c` | `0x30f8` | `0x3010` | Mode bit12=1, legacy/force mask bit13=1; SDK config0's control bit3=0 and coefficient bits7:4=1 |
| `0x600a40fc` | `0x20000000` | `0` | First ACK-dump control bit29 clear |
| `0x600a4100` | `0x20000000` | `0` | Second ACK-dump control bit29 clear |

These addresses and masks are **private, binary-derived and source-specific**,
not a documented portable PHY API. The same vendor functions repeatedly read
these words while updating them; this adapter only reads each word once after
configuration and performs no MMIO writes. It does not interpret unrelated
bits, especially bit2. The coefficient value1 is not reported as
`val_scale_cfg=1`, calibrated gain, a physical amplitude shift or automatic
scaling. Public documentation gives the opaque configuration range0–8 only.
Gain calibration and phase qualification remain absent.

The build refuses other targets/IDF versions and checks SHA256 of all vendor
inputs before enabling this private adapter. Pinned IDF commit:
`7101770dc6db2667b3c477cc31365dd1acd6db4e` (v6.0.2); Wi-Fi submodule:
`bb69e7e609c9a9a909ddd1ed175f36df2bd13801`; PHY submodule:
`59c1234e929212aec0fdda75769b759951235536`.
Exact header/library hashes are enforced in `main/c5_lltf8_sdk_pins.cmake`.

Wire v1's byte8 continues to mean **genuine configuration state matched**;
it does not promise that the public getter returned every field. Packing,
PHY validity, frequency, sanitation, native grid and feature projection are
unchanged. Unknown/mismatched readback remains blocked. Persistent diagnostics
separate the public getter's unwritten fields from the raw and masked PHY
snapshot; they emit at most three lines every five seconds. This startup
snapshot is not a continuous hardware monitor. Physical callback shape,
validity and packing still require a separate one-board check before capture.

## Live receipt lifetime and observed link fence

The initial PHY snapshot is diagnostic history, not perpetual permission.
The experimental producer reads the same three control words again at callback
entry and after filtering/serialization, before edge or UDP output. Both must
match the pinned write footprint; a cached successful startup receipt cannot
substitute for these fresh reads. No SDK API, allocation, MMIO write or lock is
added to the callback. Six fixed register reads are added per admitted callback
(three for a rejected entry); their physical cadence impact remains unmeasured.

One lock-free32-bit atomic stores observed association state in bit0 and an
event generation in the remaining31bits. Connected/disconnected/stop events
advance that generation. A callback requires a connected, identical packed
state across both receipt checks. Disconnect/reconnect ABA, a disconnected
node or PHY drift during filtering/serialization refuses output. Duplicate
events conservatively fence the crossing callback; subsequent callbacks need
fresh matching state again. Other chips/default profiles are unchanged.

The public SDK documentation places the CSI callback in the Wi-Fi task; the
pinned `wdev_csi_rx_process` calls the registered callback synchronously and
frees its temporary buffer afterward. This establishes callback execution,
not an acquisition-time association identifier. The event epoch does **not**
attest a packet's BSSID, original association, empty an upstream SDK queue or
qualify transmitter provenance. These remain unverified, as in the receiver's
existing source contract. No concurrent firmware CSI-format setter exists
after init; undocumented internal PHY behavior remains a physical test boundary.
The three separate control reads are not an atomic multi-register hardware
snapshot, and the endpoint checks are not a continuous hardware monitor.
