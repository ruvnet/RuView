# ADR-CE-016: RuvLLM CoherenceValidator Uses Sheaf Energy

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

RuvLLM's `CoherenceValidator` currently uses heuristic scoring to detect:
- Semantic inconsistency
- Factual contradictions
- Logical errors

These heuristics are:
- Pattern-based (can be fooled)
- Not mathematically grounded
- Difficult to explain

## Decision

**RuvLLM CoherenceValidator uses sheaf energy, not heuristic scores.**

Integration:
```rust
pub struct SheafCoherenceValidator {
    graph: SheafGraph,
    gate: CoherenceGate,
    inner: CoherenceValidator,  // Fallback
}
```

Process:
1. Convert context and response to sheaf nodes
2. Add edges for semantic implications
3. Compute coherence energy
4. Gate decision replaces heuristic score

## Consequences

### Benefits
- Mathematical proof of inconsistency, not pattern matching
- Explainable: can show which edges have high residuals
- Unified with Prime-Radiant governance

### Risks
- Requires embedding quality for node states
- Edge creation logic needs domain expertise

## References

- ADR-014: Coherence Engine Architecture, "RuvLLM Integration"
- ruvllm/src/quality/coherence.rs
