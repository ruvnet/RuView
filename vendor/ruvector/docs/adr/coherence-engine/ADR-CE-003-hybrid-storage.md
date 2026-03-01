# ADR-CE-003: PostgreSQL + Ruvector Unified Substrate

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

The coherence engine requires:
- Transactional authority for governance data (policies, witnesses, lineage)
- High-performance vector/graph operations for coherence computation
- Audit trail with deterministic replay

## Decision

**PostgreSQL + ruvector as unified substrate.**

| Layer | Storage | Purpose |
|-------|---------|---------|
| Governance | PostgreSQL | Policy bundles, witnesses, lineage (ACID) |
| Coherence | ruvector | Node states, edges, HNSW index, residuals |
| Audit | PostgreSQL | Event log with signatures |

## Consequences

### Benefits
- PostgreSQL: Battle-tested ACID for governance
- ruvector: Optimized for vector similarity and graph traversal
- Clear separation of concerns

### Risks
- Two systems to maintain
- Cross-system consistency requires careful transaction handling

## References

- ADR-014: Coherence Engine Architecture, Section 13
