# ADR-398: Treat nonfinite ESPHome sensor states as unavailable

- Status: proposed
- Date: 2026-10-11
- Related: ADR-373 Amendment 3 (native sensor-state semantics)

## Context

Native protobuf float states can encode NaN and infinities without setting the
missing-state flag. Positive infinity passed the distance filter and poisoned an
otherwise finite distance mean; last entity states also exposed nonfinite values
that become null only after JSON serialization, producing inconsistent reports.

## Decision

Treat a nonfinite sensor float as missing when ingesting a state. Preserve state
arrival counts, normal finite values, proto3 absent-float zero, explicit missing
flags, and binary/text state behavior. Existing summary rules then exclude the
invalid sample from means and report unavailable last state as null.

## Verification

Owned loopback native-protocol fixtures emit a 50 cm state followed by explicitly
encoded NaN, positive infinity, or negative infinity. Corrected baseline fixtures
fail on the nonfinite mean or last-state result; all fixed cases retain the 50 cm
mean, null latest state, and two arrival counts. The existing fixture controls
finite vitals, explicit missing states, and omitted proto3 zero values. No real
sensor or external service is accessed; this proves software numeric handling.
