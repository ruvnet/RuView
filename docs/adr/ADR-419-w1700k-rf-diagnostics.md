# ADR-419: W1700K diagnostics and RuField evidence boundary

Status: implementation candidate; hardware validation pending.
Date: 2026-10-14

## Decision

Use an original C consumer of the upstream MIT shared memory ABI, compact JSONL diagnostics, a local browser viewer, and a bounded RuField adapter. Spectrum quality is a separate modality and cannot contribute occupancy evidence.

Upstream: h0m3us3r/w1700k-sdr commit 0b0413e4309f0c083702fa4eb2d1c2bca4532163, host/stream/include/w1700k_stream.h. Do not copy the GPL recorder.

## Flow and invariants

1. The upstream daemon publishes native-endian IQ in ABI v1 shared memory.
2. The consumer validates geometry, reads bounded slots, checks tags around copies, and calculates RMS and int16 rail fractions.
3. JSONL retains source mode, sample rate, completion index, loss and quality reasons. Raw IQ stays on the host.
4. The browser imports records locally. Importing live-labelled data is still file replay.
5. RuField preserves source labels, produces untrusted diagnostics and excludes them from occupancy fusion.

Acceptance: synthetic ABI fixture through the real bridge, JSON validator, viewer and Rust adapter. Malformed geometry, truncation and invalid numbers fail closed. Unknown fields cannot smuggle IQ into downstream evidence.

## Configuration and timing

The header does not establish LO, gain, band or an atomic configuration generation. Caller frequency is advisory. All v1 records have config_verified=false and valid=false. Metrics remain useful for inspection; they cannot promote sensing claims.

Monotonic nanoseconds are host-local, not UTC or cross-device synchronization. Event slots consume no sample time. Unknown sample position remains null.

IQ scale is 8192. Flattening may legitimately exceed that amplitude; rail fraction counts int16 rails, not calibrated ADC clipping.

## Alternatives and limits

A second UDP receiver duplicates transport handling. Direct IQ into a CSI model violates its input contract. Upstream ci16 sidecars truncate gap indices and omit event positions, preventing exact arbitrary replay. Use shared memory and exact synthetic fixtures.

No CSI extraction, passive radar, pose, hardware accuracy lift or sustained 320 Msps benchmark is claimed. No router flashing or transmission.

## Security and release

Bound file sizes, offsets, record lengths and numeric ranges. Reject unknown diagnostic fields. Local file import performs no upload. Trust enrollment is separate from parsing; synthetic evidence stays labelled.

Publish tested source after independent review. Deploy only the viewer directory, preserving other Pages content. Roll back by reverting this change and its deployment.
