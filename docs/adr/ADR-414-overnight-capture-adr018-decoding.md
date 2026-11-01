# ADR-414: Use ADR-018 decoding for overnight capture segments

- Status: proposed
- Date: 2026-10-11
- Related: ADR-018 (wire layout), ADR-135 (empty-room corpus)

## Context

The overnight empty-room recorder claims the same ADR-018 layout as the ordinary
UDP recorder, but still reads an obsolete eight-byte header. It records the
subcarrier-count bytes as RSSI/channel, consumes the remaining header as I/Q,
and accepts unrelated or truncated datagrams. This corrupts baseline corpus
samples before downstream analysis.

## Decision

Reuse record-csi-udp.py's existing twenty-byte ADR-018 parser through a local
module import. Preserve segment naming, duration and receive workflow. The
shared parser retains receive timestamps, decodes metadata and declared
antenna-major payload, ignores trailing bytes, and rejects unsupported magic or
truncated payloads. Avoid a second copy of the packet layout and do not alter
quality, calibration or physical capture policy.

## Verification

Actual public overnight CLI writes owned temporary segment files from loopback
UDP traffic. Baseline reports the wrong RSSI, channel and payload and retains
invalid packets. Fixed tests verify declared multi-antenna amplitudes and raw IQ,
metadata, trailing-byte handling, and valid-frame continuity after invalid or
truncated datagrams. All eight harness gates run alongside these tests.
This is deterministic software packet evidence only; no hardware accuracy claim.
