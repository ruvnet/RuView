# ADR-407: Preserve interrupted serial-capture state in device summaries

- Status: proposed
- Date: 2026-10-11
- Related: ADR-373 (bounded serial pump and device readers)

## Context

The pump can emit valid HEX chunks before its process fails. readSerial retains
those useful bytes, but discards the failure whenever bytes exist. Radar and
LiDAR callers consequently present an ordinary successful capture without any
indication that the requested window was interrupted. Auto-detected radar also
silently retains only its probe if the remainder capture fails.

## Decision

Keep valid partial bytes and measurements. Mark the capture as incomplete and
include a warning when the pump process fails after producing data. Propagate
that state through both serial device summaries, including an auto-detection
remainder failure. Do not expose subprocess stderr in the warning. Existing
failures without bytes and normal complete capture behavior remain unchanged.
The flag describes transport completeness, not measurement accuracy.

## Verification

The actual Python PUMP_SCRIPT runs through the production execTool process
runner, with a temporary fixture serial module that emits valid radar or scan
bytes then raises. The public serial, radar and LiDAR readers must retain data
and report captureIncomplete. A real normally completing pump is the control.
Auto-detection also receives a normal probe followed by a failing empty pump.
All serial port operations stay inside the fixture module; no hardware opens,
physical motor commands, radio transmissions or model calls occur.
