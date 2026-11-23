# ADR-391: Retain synthetic CSI provenance in analysis input

- Status: proposed
- Date: 2026-10-11
- Related: ADR-323 (RAC1 synthetic flag), ADR-373 (CSI analysis)

## Context

The RAC1 decoder exposes the firmware's synthetic flag and the waterfall retains
it, but the kernel-analysis frame collector discarded it. Analysis of synthetic
input consequently returned the same live-CSI explanatory note as unmarked input,
preventing callers from distinguishing synthetic-inclusive estimates.

## Decision

Track whether any retained analysis frame is firmware-marked synthetic, expose
that boolean in `analysis.input.synthetic`, and explicitly describe synthetic
input in the estimate note. Frames rejected by the analysis buffer cap do not
change the provenance of the retained computation. Keep arrival-rate measurements
and raw packet diagnostics unchanged. An unmarked packet is not proof of physical
radio origin; this records an explicit upstream flag rather than certifying truth.

## Verification

The public capture producer receives protocol-valid RAC1 datagrams through owned
loopback UDP and invokes an injected deterministic analyzer. A mixed marked and
unmarked window fails the baseline provenance assertion. The fixed path retains
true for the mixed window and false for an unmarked control. This validates
software provenance propagation only, not sensing accuracy or hardware behavior.
