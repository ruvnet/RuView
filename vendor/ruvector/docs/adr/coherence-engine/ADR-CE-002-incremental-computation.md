# ADR-CE-002: Incremental Coherence Computation

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

Recomputing global coherence energy for every update is O(|E|) where |E| is edge count. For large graphs with frequent updates, this is prohibitive.

## Decision

**Incremental computation with stored residuals, subgraph summaries, and global fingerprints.**

Components:
1. **Stored residuals**: Cache per-edge residuals, update only affected edges
2. **Subgraph summaries**: Pre-aggregate energy by scope/namespace
3. **Global fingerprints**: Hash-based staleness detection

When node v changes:
1. Find edges incident to v: O(degree(v))
2. Recompute only those residuals: O(degree(v) × d)
3. Update affected subgraph summaries: O(log n)

## Consequences

### Benefits
- Single node update: O(degree × d) instead of O(|E| × d)
- Fingerprints enable efficient cache invalidation
- Subgraph summaries support scoped queries

### Risks
- Memory overhead for cached residuals
- Consistency between cache and graph requires careful management

## References

- ADR-014: Coherence Engine Architecture, Section 2
