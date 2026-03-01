# ADR-CE-009: Single Coherence Object

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

Building domain-specific coherence systems (one for AI, one for finance, one for medical) leads to:
- Duplicated effort
- Inconsistent semantics
- Maintenance burden

## Decision

**Single coherence object - once math is fixed, everything is interpretation.**

The Universal Coherence Object:
- Nodes: d-dimensional state vectors
- Edges: Restriction maps ρ_u, ρ_v
- Energy: E(S) = Σ w_e|r_e|²
- Gate: E < θ → allow

Domain-specific interpretation:
| Domain | Nodes | Edges | Residual | Gate |
|--------|-------|-------|----------|------|
| AI | Beliefs | Citations | Contradiction | Refusal |
| Finance | Trades | Arbitrage | Regime mismatch | Throttle |
| Medical | Vitals | Physiology | Clinical disagreement | Escalation |

## Consequences

### Benefits
- One implementation, many applications
- Proven math applies everywhere
- Domain experts focus on interpretation, not implementation

### Risks
- Abstraction may not fit all domains perfectly
- Requires mapping domain concepts to universal structure

## References

- ADR-014: Coherence Engine Architecture, "Universal Coherence Object"
