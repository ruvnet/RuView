# ADR-410: Require callback/frame markers for CSI monitor flow evidence

- Status: proposed
- Date: 2026-10-11
- Related: ADR-373 (serial monitor), ADR-375 (starvation diagnostics)

## Context

The monitor counted any line containing the ESP32 csi_collector log tag as a
CSI callback. Current firmware uses that tag for early configuration, startup,
self-ping failures and other metadata as well as actual callbacks. A startup
log with no callback consequently returned ok=true and falsely certified flow.
The source's actual callback line uses `CSI cb #`; Realtek uses
`RUVIEW_CSI: frame #`.

## Decision

Count only those explicit callback/frame markers as flow evidence. Keep the
existing startup-upgrade metadata and independent buffer-starvation detection.
The existing count remains a count of observed rate-limited log markers, not a
radio callback-rate measurement. No UART, reset or hardware control behavior
changes. Older log formats with only configuration tags cannot certify flow.

## Verification

The public SDK monitor runs the actual Python monitor script in a real child
process. An isolated serial module supplies current firmware startup/self-ping
messages; the baseline falsely succeeds, and the fix returns no-flow diagnostics.
Mixed startup plus ESP32/Realtek frame markers counts only two observations.
A real frame plus a starvation message still fails as csi_buffer_starvation.
Existing monitor rendering and Python fixture controls run too. No physical
serial port opens or node resets occur.
