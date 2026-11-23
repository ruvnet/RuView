# ADR-409: Align SDK device argument types with supported runtime options

- Status: proposed
- Date: 2026-10-11
- Related: ADR-369 (typed SDK), ADR-373 (device readers), ADR-378 (spectrum)

## Context

The SDK forwards device arguments to the current tool registry, but its shipped
TypeScript declarations still describe earlier options. A real TypeScript
consumer cannot select ESPHome by source/host/API port, request ESP32 spectrum
rows, or set an iPhone depth packet cap. Each workflow already works through the
same public SDK at runtime. Explicit serial source is missing too.

## Decision

Describe mmWave arguments as serial (the existing default, requiring port) or
ESPHome (requiring host). Add the existing optional spectrum parameters and
iPhone max_frames. Keep numeric and enum constraints, and all existing serial
call shapes. This changes declarations only; no reader or tool behavior changes.
Group these omissions as one declaration-drift fix rather than separate fields.

## Verification

A TypeScript consumer compiled with strict NodeNext checks fails on the valid
runtime options before the fix and succeeds after it. Its expected-error controls
retain host/port requirements, numeric parameter types, and known backends. The
same public SDK is exercised with real owned TCP ESPHome, UDP CSI, and WebSocket
depth producers: numeric state, spectrum bins, and packet-cap termination all
work before and after the declaration change. No hardware or external service
is involved. The fixture can be replayed with an installed TypeScript compiler:

```
tsc --strict --noEmit --target ES2022 --module NodeNext --moduleResolution NodeNext test/fixtures/device-types.mts
```
