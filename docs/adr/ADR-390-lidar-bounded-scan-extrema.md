# ADR-390: Reduce LiDAR scan extrema without variadic argument limits

- Status: proposed
- Date: 2026-10-11
- Related: ADR-373 (device diagnostics)

## Context

`readRplidar` accepts up to 4 MiB of serial scan bytes. Its summary passed every
valid distance as an argument to `Math.min` and `Math.max`. A valid 1 MiB scan
containing 200,000 measurement nodes exceeds the JavaScript argument-stack limit
and throws `RangeError` instead of returning diagnostics.

## Decision

Reduce extrema with a loop and constant auxiliary state. Retain current median,
zero-distance filtering, coverage, units, and empty-scan behavior. This avoids
runtime-specific argument limits across the full supported capture size.

## Verification and limits

An offline serial-pump dependency supplies protocol-valid bytes to the public
`readRplidar` producer. The 3-point control passes on the baseline; the 200,000
point fixture fails with `RangeError`. Both pass after this change with exact
minimum, median, maximum, and coverage checks. Existing device tests provide
empty and mixed-input controls. The fixture uses no serial device or motor and
proves software decoding/aggregation only, not real hardware performance.
