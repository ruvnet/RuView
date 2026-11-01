# ADR-416: Validate alignment CLI window parameters before processing captures

- Status: proposed
- Date: 2026-10-11
- Related: ADR-079 (ground-truth pairing)

## Context

The public ground-truth alignment CLI parses window and numeric options without
validation. A zero window frame count makes the windowing loop advance by zero
and grow indefinitely. Negative counts also cannot advance correctly, and
fractional or partially numeric values are silently truncated. Invalid camera
counts, confidence thresholds and time values can bypass matching checks or
produce misleading paired output.

## Decision

Parse complete numeric values and validate before reading or writing captures.
Require positive safe integer frame counts, finite nonnegative window duration,
confidence in [0,1], and finite signed clock offsets. Retain zero expansion and
zero confidence as supported controls. Report the offending option and exit2
for invalid inputs. Preserve valid windowing and pairing calculations.

## Verification

Actual CLI subprocesses process owned tiny feature/camera recordings. The
baseline zero/negative count processes are bounded by a one-second timeout and
32MiB heap cap; fractional/malformed/range cases lack the expected early error.
The fixed CLI rejects each invalid option before output creation and retains
complete valid windows, zero expansion/confidence and a signed finite offset.
An initial fixture passed negative values as separate tokens, which Node's
argument parser already rejects; that attempt is retained but invalidated.
Final regressions use documented --option=value syntax. All eight harness gates
run. No model training, camera, physical capture or hardware operation occurs.
