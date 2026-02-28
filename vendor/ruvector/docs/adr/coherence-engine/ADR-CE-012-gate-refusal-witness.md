# ADR-CE-012: Gate = Refusal Mechanism with Witness

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

When coherence energy exceeds threshold, the system must refuse action. This refusal needs to be:
- Deterministic (same inputs → same decision)
- Auditable (why was it refused?)
- Provable (cryptographic witness)

## Decision

**Gate = refusal mechanism with witness - every refusal is provable.**

Gate evaluation produces:
```rust
pub struct GateDecision {
    pub allow: bool,
    pub lane: ComputeLane,
    pub witness: WitnessRecord,
    pub denial_reason: Option<String>,
}
```

The WitnessRecord includes:
- Energy snapshot at decision time
- Policy bundle that defined thresholds
- Hash chain to previous witness
- Content hash for integrity

## Consequences

### Benefits
- Every refusal has cryptographic proof
- Can reconstruct exactly why any decision was made
- Compliance-ready audit trail

### Risks
- Witness storage overhead
- Must handle witness retrieval at scale

## References

- ADR-014: Coherence Engine Architecture, Section 3
- ADR-CE-005: First-Class Governance Objects
