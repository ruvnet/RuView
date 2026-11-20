# ADR-405: Complete iPhone LiDAR captures when the relay stream closes

- Status: proposed
- Date: 2026-10-11
- Related: ADR-373 (bounded device readers)

## Context

The iPhone reader completes at its packet limit, timeout, or pre-open failure.
After an opened WebSocket closes normally, however, it keeps the capture timer
running despite having no remaining source of frames. A one-frame stream that
closes immediately waits the entire requested window; an empty normal stream
also delays its no_frames result. This wastes the user's capture time and uses
the idle post-close interval in its reported receive rate.

## Decision

Finish on every close event. Retain accepted depth statistics. Preserve the
existing relay_closed refusal for abnormal empty streams, pre-open connection
failure, and max_frames termination. A normal empty stream still returns
no_frames; a stream with valid depth frames remains a successful capture. The
requested seconds field retains its existing meaning.

## Verification

An owned loopback HTTP server performs a real WebSocket upgrade and sends a
valid one-pixel depth frame followed by a normal close handshake. The public
reader must return promptly instead of waiting its two-second capture timeout.
A normal empty stream and abnormal empty code1008 stream check refusal behavior;
a max_frames=1 control checks existing early termination. These are transport
fixtures, not iPhone hardware or depth-accuracy evidence.
