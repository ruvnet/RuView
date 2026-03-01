# ADR-CE-013: Not Prediction

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

Most AI systems try to predict what will happen. This is fundamentally limited:
- Future is uncertain
- Predictions can be confidently wrong
- No structural guarantees

## Decision

**Not prediction - system shows safe/unsafe action, not what will happen.**

The coherence engine answers a different question:

| Prediction Systems | Coherence Systems |
|--------------------|-------------------|
| "What will happen?" | "Does the world still fit together?" |
| Probabilistic confidence | Mathematical consistency |
| Can be confidently wrong | Knows when it doesn't know |
| Trust the model | Trust the math |

The coherence field shows:
- Where action is safe (low energy)
- Where action must stop (high energy)

It does NOT predict outcomes.

## Consequences

### Benefits
- Honest uncertainty: "I don't know" is a valid answer
- No false confidence in predictions
- Structural guarantees, not statistical ones

### Risks
- Users may expect predictions
- Requires education on coherence vs. confidence

## References

- ADR-014: Coherence Engine Architecture, "The Coherence Vision"
