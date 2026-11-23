# ADR-392: Return single valid frames for explicitly selected radar models

- Status: proposed
- Date: 2026-10-11
- Related: ADR-373 (host device diagnostics)

## Context

The serial radar reader requires two valid frames before auto-detecting a model.
That confidence threshold was also applied to an explicitly selected model,
rejecting a valid short capture with one decoded presence or vital report as
`no_valid_frames`. The caller already supplied the model and baud selection.

## Decision

Require two frames for auto detection and one for an explicitly selected model.
Retain the empty-capture failure, checksums, parsing, capture permissions, and
all reported values. A single decoded frame is a successful software read and
is not evidence of clinical accuracy or sustained radar performance.

## Verification

An offline serial-pump fixture supplies one checksum-valid MR60BHA2 presence
report through the public reader. Explicit model selection fails on the baseline
and returns one frame with presentFraction=1 after the change. The same input in
auto mode remains refused. Existing multi-frame and invalid-input device tests
remain controls. No serial port or hardware was opened by this verification.
