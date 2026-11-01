# ADR-401: Validate auxiliary node datagrams before counting decoded reports

- Status: proposed
- Date: 2026-10-11
- Related: ADR-069 (features), ADR-039 (compression), feature-state and WASM firmware formats

## Context

Known auxiliary magics were sufficient to count a datagram as decoded. A
four-byte feature, compressed, feature-state, or WASM prefix created a phantom
node and successful diagnostic counts. Corrupt feature-state CRCs and truncated
variable payloads were also accepted, hiding actual data corruption.

## Decision

Validate firmware-defined payload envelopes before retaining their existing
metadata-only report shape: features require their 48-byte record, compressed
reports require a 10-byte header and declared non-empty payload, feature-state
requires the fixed 60-byte record and its IEEE CRC, and WASM output requires
an 8-byte header plus five bytes for each of at most 16 declared events. Keep
raw auxiliary payloads out of the response and retain existing kind counters.

Source contracts are `edge_processing.h`/`send_compressed_frame` in
`edge_processing.c`, `rv_feature_state.h`, and the variable-length producer in
`wasm_runtime.c`. This validates envelope integrity without claiming to decompress
CSI or interpret feature/event semantics.

## Verification

Owned loopback UDP drives the public capture with four complete envelope controls,
four magic-only prefixes, one corrupt CRC, and three truncated payloads. The
baseline counts all twelve as decoded and hides corruption. The correction counts
four decoded and eight malformed, with one valid node and exact kind counters.
Existing CSI, vitals, and other packet tests remain controls. No firmware or real
sensor was exercised; the claim is software format and diagnostic correctness.
