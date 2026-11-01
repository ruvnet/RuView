# ADR-399: Select the busiest CSI stream using total arrivals

- Status: proposed
- Date: 2026-10-11
- Related: ADR-373 (bounded live CSI analysis)

## Context

The analysis collector advertises selection of the busiest node's dominant shape.
It ranks buffers by their retained frame length. Once multiple streams reach the
analysis buffer cap, their retained lengths tie and insertion order wins even
when a later stream received substantially more frames. A 70-frame first node
therefore beats a 100-frame second node under a 64-frame cap.

## Decision

Rank candidate streams by retained frames plus their existing dropped-over-cap
counter. Preserve the frame cap, first-window sample rate, explicit node filter,
and deterministic insertion order for actual arrival-count ties. The analyzer
still consumes only the retained bounded samples; ranking does not invent data.

## Verification

The public capture receives two protocol-valid node streams through owned
loopback UDP. The baseline selects the 70-frame node despite the second node's
100 arrivals. The correction selects node 2 and reports 64 retained/36 omitted
frames. An explicit node-1 control still selects node 1 and reports 64/6.
Existing uncapped analysis and decoding tests remain controls. This is software
stream-selection verification, not hardware throughput or sensing accuracy.
