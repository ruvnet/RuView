# ADR-CE-011: Residual = Contradiction Energy

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

The edge residual r_e = ρ_u(x_u) - ρ_v(x_v) measures local mismatch. This mathematical quantity needs a universal interpretation across domains.

## Decision

**Residual = contradiction energy - universal interpretation across domains.**

The residual represents:
- **AI Agents**: Logical contradiction between belief and evidence
- **Finance**: Regime mismatch between positions
- **Medical**: Clinical disagreement between vitals and diagnosis
- **Robotics**: Physical impossibility between sensor and plan
- **Security**: Authorization violation between permission and action

The weighted residual norm |r_e|² is always "how much these two things disagree."

## Consequences

### Benefits
- Universal semantics: "disagreement" makes sense everywhere
- Quantitative: larger residual = bigger problem
- Localizable: can identify which edges contribute most

### Risks
- Restriction map design determines what "disagreement" means
- Poor maps give meaningless residuals

## References

- ADR-014: Coherence Engine Architecture
- ADR-CE-009: Single Coherence Object
