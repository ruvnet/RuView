# ADR-386: Recover CSI packet loss by missing sequence identity

Status: Proposed; maintainer acceptance pending

## Context

The RF capture diagnostics report CSI packet loss and account for UDP reordering. The existing tracker decrements its gap count for any late sequence while gaps exist. A duplicated recovery or previously received packet therefore erases a different missing packet. For example, sequences 100, 103, 101, 101 leave 102 missing but currently report zero loss. Accurate loss diagnostics matter when distinguishing transport problems from healthy sensing input.

## Decision

Retain the identities of counted missing sequences within the existing 1024-sequence reorder window. Recover a gap only when deleting its identity succeeds, so a duplicate cannot recover it twice. Prune identities outside the window without erasing their historical gap counts. A confirmed counter reset clears recovery identities but preserves accumulated loss. Keep the existing frame count/rate, loss denominator, stray/reset classification and packet format unchanged.

## Alternatives

A duplicate-only set cannot distinguish packets observed before the capture baseline from genuinely missing ones. Keeping all missing identities indefinitely is unnecessary and could grow without bound.

## Verification

MEASURED software fixtures use byte-accurate RAC1 envelopes, the public capture API and real loopback UDP. Regressions cover duplicated recoveries, already-received packets, pre-capture packets and distinct valid recoveries. A direct public parser/NodeStats control checks bounded bookkeeping and reset semantics. Existing sequence/stray/reset tests remain applicable. This is a software diagnostic correction, not physical RF, device throughput or physiological accuracy validation.

## Consequences

Loss no longer disappears when unrelated late packets arrive. Recovery remains limited to the established reorder window; older gaps stay counted. Stored identities are bounded per node.
