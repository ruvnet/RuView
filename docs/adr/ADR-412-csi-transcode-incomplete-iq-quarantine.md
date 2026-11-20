# ADR-412: Quarantine incomplete recorded I/Q pairs during transcode

- Status: proposed
- Date: 2026-10-11
- Related: ADR-096 and ADR-097 (rvCSI capture bridge)

## Context

The public ESP32 JSONL converter is the documented capture fixture bridge. Its
contract drops rejected frames. It currently floors odd byte lengths, silently
accepting a truncated I/Q pair as a shorter valid frame; malformed hex instead
aborts the entire capture and loses later valid frames. The existing rej_len
counter is never used.

## Decision

Reject invalid hex and odd byte lengths before constructing numeric samples.
Increment the existing rej_len counter once for each rejected record and
continue processing later frames. Preserve complete int8 pair conversion,
quality scoring, timestamps, and output format. Do not infer missing samples or
change adapter calibration or hardware policy.

## Verification

Actual public Python CLI processes owned temporary JSONL captures with valid
frames before and after each damaged record. The baseline accepts the odd pair
and aborts on invalid hex. Fixed runs retain the two complete frames with
contiguous frame IDs and truthful counters. A signed-IQ control verifies
amplitude and real/imaginary orientation. All eight harness gates run alongside
these subprocess tests. This is offline recording evidence only.
