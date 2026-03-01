# ADR-CE-001: Sheaf Laplacian Defines Coherence Witness

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

Traditional AI systems use probabilistic confidence scores to gate decisions. These scores:
- Can be confidently wrong (hallucination)
- Don't provide structural guarantees
- Are not provable or auditable

## Decision

**Sheaf Laplacian defines coherence witness, not probabilistic confidence.**

The coherence energy E(S) = Σ w_e|r_e|² provides a mathematical measure of structural consistency where:
- r_e = ρ_u(x_u) - ρ_v(x_v) is the edge residual
- w_e is the edge weight
- Zero energy means perfect global consistency

## Consequences

### Benefits
- Mathematical proof of consistency, not statistical guess
- Every decision has computable witness
- Residuals pinpoint exact inconsistency locations

### Risks
- Restriction map design requires domain expertise
- Initial setup more complex than confidence thresholds

## References

- Hansen & Ghrist (2019), "Toward a spectral theory of cellular sheaves"
- ADR-014: Coherence Engine Architecture
