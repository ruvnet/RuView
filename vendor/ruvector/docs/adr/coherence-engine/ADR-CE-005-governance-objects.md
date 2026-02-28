# ADR-CE-005: First-Class Governance Objects

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

Governance decisions (thresholds, policies, approvals) must be:
- Versioned and traceable
- Signed by authorized parties
- Immutable once approved
- Addressable for reference in witnesses

## Decision

**Governance objects are first-class, immutable, addressable.**

Three governance object types:

1. **PolicyBundle**: Versioned threshold configurations
   - Signed by required approvers
   - Content-addressed (ID = hash of contents)
   - Immutable once created

2. **WitnessRecord**: Proof of gate decisions
   - Links to PolicyBundle used
   - Chains to previous witness (hash chain)
   - Content-addressed

3. **LineageRecord**: Provenance of writes
   - Links to authorizing witness
   - Tracks causal dependencies
   - Enables "why did this change?" queries

## Consequences

### Benefits
- Complete audit trail for compliance
- Multi-party approval for sensitive changes
- Content addressing prevents substitution attacks

### Risks
- Cannot modify bad policies (must create new version)
- Storage overhead for immutable objects

## References

- ADR-014: Coherence Engine Architecture, Section 4
