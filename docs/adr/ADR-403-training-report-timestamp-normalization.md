# ADR-403: Normalize chronological report timestamps without truthiness

- Status: proposed
- Date: 2026-10-11
- Related: ADR-371 (offline training evidence gate)

## Context

The gate used `Date.parse(value) || numericFallback`. Valid epoch-zero ISO times
parse to zero and were then treated as missing. Documented numeric millisecond
boundaries were first interpreted as calendar strings by Date.parse, so 5 ms and
30 ms could be misordered and falsely refused as temporal leakage.

## Decision

Retain finite numeric boundaries as milliseconds and parse string boundaries
once. Test finiteness explicitly rather than using truthiness. Preserve refusal
of invalid/missing dates and non-increasing train/test chronology. The tool schema
continues accepting its existing ISO string input; the public training export
retains its documented numeric input support.

## Verification

The actual SDK evaluates an epoch-zero ISO boundary with a later test timestamp.
The public training export evaluates numeric 5 ms and 30 ms boundaries. Both
baseline cases are falsely refused and pass after normalization. Equal-time and
invalid-date controls remain refused. Fixtures are explicitly synthetic; no
training, model calls, dataset claims, or publication is performed by these tests.
