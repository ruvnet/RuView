# ADR-CE-006: Coherence Gate Controls Compute Ladder

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

Not all coherence violations require the same response. A minor transient spike differs from sustained structural breakdown. The system needs graduated responses.

## Decision

**Coherence gate controls explicit compute ladder: Reflex → Retrieval → Heavy → Human.**

| Lane | Latency | Trigger | Action |
|------|---------|---------|--------|
| 0: Reflex | <1ms | E < θ_reflex | Proceed, local update |
| 1: Retrieval | ~10ms | θ_reflex ≤ E < θ_retrieval | Fetch evidence, lightweight reasoning |
| 2: Heavy | ~100ms | θ_retrieval ≤ E < θ_heavy | Multi-step planning, spectral analysis |
| 3: Human | Async | E ≥ θ_heavy or persistent | Escalate to human, block action |

## Consequences

### Benefits
- Most operations stay fast (Lane 0)
- Graduated response matches severity
- Human escalation for truly difficult cases
- Every escalation has witness

### Risks
- Threshold tuning requires domain knowledge
- Over-sensitive thresholds cause unnecessary escalation

## References

- ADR-014: Coherence Engine Architecture, Section 3
- ADR-CE-014: Reflex Lane Default
