# ADR-394: Apply strict SDK errors to diagnostic reports

- Status: proposed
- Date: 2026-10-11
- Related: ADR-369 (SDK operator interface)

## Context

A strict SDK instance throws `RuViewError` when an operation returns `ok:false`.
The doctor helper directly invokes the diagnostic producer to retain injectable
operator dependencies, bypassing that shared result check. Failed diagnostics
therefore silently resolved even when the caller selected strict mode.

## Decision

Extract the existing result check and apply it to both registry calls and doctor
reports. Preserve injected dependencies and retain the original structured report
on the thrown error. Non-strict calls and successful diagnostics continue to
resolve reports; the existing dry-run exception remains unchanged.

## Verification

The public SDK doctor runs a deterministic offline invalid-protocol diagnostic.
The baseline fails the promised strict rejection. The correction checks the exact
error class, tool identifier, and failed report; controls verify non-strict failure
reports and successful strict reports. No external service or device is accessed.
