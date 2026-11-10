# Local C5 L-LTF8 producer format claim v1

This is a local experimental format contract, not an official firmware release,
hardware attestation, calibrated phase or sensing-accuracy result. The previous
C5 profile zero-initialized `lltf_bit_mode`; in ESP-IDF 6.0.2 that selects 12-bit
components, not the signed int8 packing assumed by ADR-018. Its observed
106-byte callbacks therefore do not establish 53 native complex int8 samples.

Primary references: [ESP-IDF v6.0.2 CSI configuration](https://github.com/espressif/esp-idf/blob/v6.0.2/components/esp_wifi/include/esp_wifi_he_types.h),
[C5 vendor features](https://github.com/espressif/esp-idf/blob/v6.0.2/docs/en/api-guides/wifi-vendor-features.rst),
and [Espressif's packing clarification](https://github.com/espressif/esp-idf/issues/18982#issuecomment-5420118269).

The new opt-in profile requests only legacy/forced L-LTF with
`lltf_bit_mode=1`, reads the actual configuration back through the SDK, and
withholds CSI if that readback differs. A callback can carry this claim only
when its original metadata reports non-HT OFDM (`cur_bb_format=1`, 11g/11a),
no secondary channel, primary 20 MHz, valid channel estimate and exactly 106
bytes. Rejected metadata is logged at a bounded rate without raw samples.
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
| 8 | SDK configuration readback matched, 1 |
| 9 | Original `cur_bb_format`, required 1 |
| 10 | Original secondary-channel field, required 0 |
| 11 | Original bandwidth field, required 0 (20 MHz) |
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
