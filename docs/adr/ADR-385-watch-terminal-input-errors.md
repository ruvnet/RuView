# ADR-385: Stop watch mode on permanent input errors in every output mode

Status: Proposed; maintainer acceptance pending

## Context

The CLI's `esp32 --watch` repeats short capture windows. Permanent bind/port errors stop the terminal view, but the piped/JSON branch continues before reaching that check. Schema-level invalid arguments also never stop either branch. Actual CLI children with a fractional UDP port or rejected bind value repeatedly emit errors despite having no usable capture configuration. These fixtures fail before opening a socket or accessing a device.

## Decision

Classify existing permanent port/bind errors and schema-invalid arguments before selecting output mode. Emit the error once and return exit status 1 for both terminal and JSON output. Keep repeated capture behavior for nonterminal results, including windows with no CSI packets; retain the normal watch output contract.

## Alternatives

Adding a delay would still repeatedly retry an unchanged invalid configuration. Stopping every unsuccessful capture would incorrectly terminate legitimate empty capture windows.

## Verification

MEASURED bounded actual CLI regression checks piped `--watch --json` with invalid inputs, one structured failure and nonzero exit. No network socket, hardware, external provider or live capture is exercised. Whole-package validation receipts accompany the final revision.
