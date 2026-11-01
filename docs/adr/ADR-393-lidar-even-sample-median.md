# ADR-393: Use the arithmetic median for even LiDAR samples

- Status: proposed
- Date: 2026-10-11
- Related: ADR-373 (range and depth diagnostics)

## Context

The shared LiDAR median helper selected the upper middle sample for even-sized
sets. The public depth reader reported 4 metres for confident depths of 1 and
4 metres, and the serial scan reader likewise returned 4000 mm. Fields named
median should report the arithmetic median, 2.5 metres or 2500 mm respectively.

## Decision

Average the two central sorted samples for even counts and preserve the central
sample for odd counts. Empty sets remain null. Existing distance/confidence
filtering and metre/millimetre conversion remain in the public producers.
Update existing even-sample expectations to reflect the corrected numerical
contract; this intentionally changes these statistics for even counts.

## Verification

Offline protocol fixtures enter both public readers through their existing
transport injection boundaries. The baseline fails two even-sample cases and
passes an unsorted odd-sample control. The correction verifies both units and
zero-depth exclusion. No real LiDAR, serial device, or radar was used, and this
software calculation does not prove measurement accuracy.
