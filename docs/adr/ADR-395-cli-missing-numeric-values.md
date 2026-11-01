# ADR-395: Preserve missing numeric CLI values for schema rejection

- Status: proposed
- Date: 2026-10-11
- Related: ADR-369 (operator CLI)

## Context

The common device/training CLI flag adapter coerces values with `Number`. A bare
`--node-id` is parsed as boolean true and becomes node 1; `--node-id=` becomes
node 0. Both silently select a real node rather than reporting the missing
numeric argument. The same adapter handles capture deadlines and sample limits.

## Decision

Retain boolean and empty-string values in the common schema argument adapter so
existing numeric schemas reject them. Convert explicit non-empty numeric values
as before, including zero. Keep presentation flags and tool schemas unchanged.
This change is limited to verbs using the common snake-case argument adapter.

## Verification

Execute the actual CLI process with both missing-value forms and explicit zero.
An intentionally invalid UDP port ensures argument validation completes before
opening a socket. Baseline errors omit node_id for both missing forms; the fixed
CLI reports those numeric type errors while preserving the explicit zero control.
This is offline CLI/schema verification, not a radio capture.
