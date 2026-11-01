# ADR-418: Handle expected resets in owned ESPHome rejection fixtures

- Status: proposed
- Date: 2026-10-11
- Related: ADR-406 (ESPHome fixture-driven role validation)
- Promotion: extend existing ESPHome PR #2221; no new publication unit

## Context

The encrypted/password test fixtures write while the reader deliberately rejects
and closes the connection. The server socket can then emit ECONNRESET after the
test ends. With no socket error listener, that expected fixture event produces
an uncaught exception and intermittent unrelated full/security gate failures.
Converter and alignment failures retained locally show this exact test event.

## Decision

Handle ECONNRESET only in the owned encrypted/password fixture modes. Continue
throwing any unexpected error code and any reset in ordinary fixture mode. Do
not change the production reader, its failure reports or its network policy.

## Verification

Actual public reader connects to owned TCP fixtures and returns the expected
rejection report. A controlled socket reset event reproduces the missing error
lifecycle deterministically: baseline throws, fixed modes accept the reset.
Negative controls retain failures for EIO in rejection mode and ECONNRESET in
ordinary mode. Existing roles and public reader tests, plus all eight gates,
run on this revision. This is an explicitly injected software fixture event,
not physical device or hardware evidence.
