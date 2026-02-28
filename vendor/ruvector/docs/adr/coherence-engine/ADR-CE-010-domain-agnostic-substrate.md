# ADR-CE-010: Domain-Agnostic Nodes and Edges

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

To support multiple domains with a single substrate, the node and edge types must be generic enough to represent:
- AI agent beliefs and citations
- Financial trades and market dependencies
- Medical vitals and physiological relationships
- Security identities and policy rules

## Decision

**Domain-agnostic nodes/edges - facts, trades, vitals, hypotheses all use same substrate.**

Node structure:
```rust
pub struct SheafNode {
    pub id: NodeId,
    pub state: Vec<f32>,      // Fixed-dimension embedding
    pub metadata: Metadata,   // Domain-specific tags
    pub updated_at: Timestamp,
}
```

Edge structure:
```rust
pub struct SheafEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub weight: f32,
    pub rho_source: RestrictionMap,
    pub rho_target: RestrictionMap,
}
```

Domain mapping happens in metadata and restriction map design.

## Consequences

### Benefits
- Single codebase for all domains
- Type safety through metadata validation
- Restriction maps encode domain semantics

### Risks
- Embedding dimension must be chosen carefully
- Metadata schema needs governance

## References

- ADR-014: Coherence Engine Architecture, Section 1
- ADR-CE-009: Single Coherence Object
