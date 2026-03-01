# ADR-CE-004: Signed Event Log with Deterministic Replay

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

For audit, debugging, and compliance, the system must support:
- Complete reconstruction of any past state
- Verification that events were not tampered with
- Replay for testing and analysis

## Decision

**Signed event log with deterministic replay.**

Every event is:
1. Assigned a monotonic sequence ID
2. Serialized with timestamp and payload
3. Signed with Blake3 hash including previous event's signature (chain)
4. Stored append-only in PostgreSQL

Replay:
- Start from genesis or checkpoint
- Apply events in sequence order
- Deterministic: same events → same state

## Consequences

### Benefits
- Tamper-evident: any modification breaks the hash chain
- Complete auditability: reconstruct any historical state
- Debugging: replay and inspect at any point

### Risks
- Storage grows indefinitely (mitigated by checkpoints)
- Replay time scales with history length

## References

- ADR-014: Coherence Engine Architecture, Section 13
