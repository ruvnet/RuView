# ADR-402: Preserve synthetic provenance for the whole CSI shape window

- Status: proposed
- Date: 2026-10-11
- Related: ADR-323 (RAC1 flags), ADR-373 (per-shape node diagnostics)

## Context

A normal capture's shape summary averages all frames but reports the synthetic
flag from only its latest packet. A marked frame followed by an unmarked frame
therefore contributes to the mean while removing the synthetic label shown by
normal CLI diagnostics. The waterfall already preserves a window-wide flag;
kernel-analysis input has a separate collector and provenance boundary.

## Decision

Track whether any frame in each statistical shape window is firmware-marked
synthetic and report that flag alongside the window mean/count. Preserve the
latest packet's other descriptor fields and keep provenance independent for each
shape. An unmarked window still reports false; that is not certification of its
physical origin. No capture or hardware authority changes.

## Verification

The public ordinary capture receives a marked RAC1 frame then an unmarked frame
through owned loopback UDP. The baseline reports two aggregated frames and false
synthetic provenance. The correction reports true; an entirely unmarked control
remains false. Existing shape and source statistics remain controls. This verifies
software window provenance, not sensing accuracy or authentic radio origin.
