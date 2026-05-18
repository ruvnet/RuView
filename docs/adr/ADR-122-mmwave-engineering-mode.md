# ADR-122 — HLK-LD2402 Engineering Mode (per-gate energies, mmWave HR)

**Status**: Accepted.
**Date**: 2026-05-18
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/mmwave.rs`
(extended), `v2/scripts/probe_ld2402_engineering.py` (new),
`ui/raw.html` (pills updated). Builds on ADR-121.

## Context

ADR-121 wired up the HLK-LD2402 in **Normal Mode** — ASCII
`distance:<cm>\r\n` lines at ~6 Hz. That gave us a live distance pill
and a passable breathing estimate (chest movement modulates range by
5–10 mm, visible as cm-bin flicker), but **no heart rate** — cardiac
chest displacement (~0.3–0.5 mm) is well below the cm quantisation
step, so heartbeat is invisible in the distance time-series.

The module supports a richer **Engineering Mode** that streams per-
range-gate amplitude at the same cadence. ESPHome's driver and the
Hi-Link command spec gave enough information to reverse-engineer it.

## Decisions

### D1 — Switch into Engineering Mode at startup

On `spawn_reader` the host sends three commands in sequence:

1. `0x00FF` — enable config mode (payload `01 00`)
2. `0x0012` — set work mode (payload `00 00 04 00 00 00`, mode = 0x04)
3. `0x00FE` — disable config mode → data stream resumes (now binary)

Each command frame is `FD FC FB FA <len LE> <cmd LE> <data...> 04 03 02 01`.
If any step errors out we log a warning and fall back to the
ADR-121 ASCII Normal-Mode parser, so distance keeps working even when
the radar is in an inconsistent state.

### D2 — Binary frame layout

Engineering frames at ~6 Hz, 141 bytes total:

```text
F4 F3 F2 F1   data header
LL LL         payload length (LE u16, = 131)
01            frame type (engineering)
DD DD         distance in cm (LE u16)
00 × 8        reserved
<15 × u32 LE> motion-gate energies (gate 0 = 0–0.7 m, …)
<15 × u32 LE> micromotion-gate energies (same range bins)
F8 F7 F6 F5   footer
```

The ESPHome project documents `0x84` as the engineering type byte,
but our firmware emits `0x01` — likely a firmware revision difference.
The parser only accepts `0x01` for now.

### D3 — Don't mix ASCII and binary drains in one loop

First-cut implementation ran both the binary frame parser and the
ASCII line drain unconditionally. Since the binary payload contains
arbitrary bytes (including `0x0A`), the ASCII drain destroyed ~80 %
of partial frames mid-buffer, dropping the effective parse rate to
1.3 Hz. The fix is to track an `engineering_mode` flag set after the
enable sequence succeeds; ASCII drain only runs in the fallback path.

After this fix, frame parsing matches the raw byte rate exactly
(~6.1 Hz observed live).

### D4 — Target-gate selection for HR extraction

The micromotion-gate at the target's range is where the cardiac
signal lives — that's the bin whose backscatter is modulated by
chest-wall displacement. Three-tier selection:

1. **Distance-based** — bracket `distance_cm` into a 0.7 m gate;
2. **Mid-range micro-peak** — if that gate's micro energy is zero
   (stale distance, wrong guess), pick the strongest micro gate in
   `[1, 14)`. Gate 0 is dominated by near-field clutter; the last
   gate is usually empty.
3. **Default** — gate 1 if all else fails (most common seated-operator
   torso distance).

### D5 — HR via bandpass + FFT on micro-gate log-energy history

Per-gate micromotion energies are pushed into 30-s ring buffers
(180 samples at 6 Hz). We log-compress (`ln(energy + 1)`) to suppress
the dynamic range of the raw u32, then run a Hann-windowed bandpass
(0.8–2.0 Hz = 48–120 BPM) + radix-2 FFT peak search on the target-
gate buffer. Confidence is the peak-to-band-mean ratio, mapped to
[0,1] the same way as ADR-021's VitalSignDetector.

Breathing keeps using the distance time-series via the shared
detector — that signal is strong enough not to need per-gate
selection.

### D6 — Diagnostic probe script kept in tree

`v2/scripts/probe_ld2402_engineering.py` does the same enable
sequence and dumps the first N frames as annotated hex. Useful for
verifying the wire format on new firmware revisions, and would have
saved an hour during this ADR's development if it had existed first.

## Verified Acceptance

Live with the module attached, target ~1.5 m away (seated):

```
$ curl :8080/api/v1/mmwave/vitals
{"available":true,
 "vital_signs": {
   "breathing_rate_bpm":   13.06,
   "breathing_confidence": 0.37,
   "heart_rate_bpm":       75.93,
   "heartbeat_confidence": 0.63
 },
 "buffer_status": {"breathing_capacity":180,"breathing_samples":180}}
```

Server logs:

```
ADR-121 mmWave reader: opened /dev/cu.usbserial-1140 @ 115200
ADR-122 mmWave: Engineering Mode enabled (per-gate energies @ 6 Hz)
```

In the UI, both pills now show two values:

```
🫁 📶 — BPM ·  | 📡 13.0 BPM · 37%      норма 12–20
💓 📶 — BPM ·  | 📡 76 BPM · 63%        норма 60–100
```

## Out of Scope / Follow-ups

* **Calibration against ground-truth pulse**. The 75 BPM value
  agrees with the seated operator's actual rest pulse to within
  ±5 BPM but hasn't been benchmarked against a chest-strap monitor
  across multiple subjects or activity levels.

* **Multi-target handling**. The current target-gate selector picks
  one gate. With two people in the field the algorithm picks
  whichever has the stronger backscatter; the other person's HR is
  lost.

* **Engineering Mode is sticky**. After this server runs, the module
  remains in engineering mode until something explicitly switches it
  back. The next ADR-121 ASCII consumer (an external tool) would
  receive binary garbage. Add a clean-shutdown step that issues
  `set_mode(0x64)` if we ever wire up signal handling.

* **Fusion with WiFi-CSI**. Now we have HR from two independent
  modalities — weighted-average or disagreement-flag could improve
  reliability. Probably ADR-123.

## References

* ADR-021 — WiFi-CSI vital signs detector (shared FFT/bandpass).
* ADR-121 — HLK-LD2402 Normal-Mode integration.
* ESPHome HLK-LD2402 driver (`Mc-Joung/hlk_ld2402_esphome`) — main
  reference for the command opcodes and frame envelope.
