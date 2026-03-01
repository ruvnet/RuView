# ADR-CE-008: Multi-Tenant Isolation

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

Enterprise deployments require multiple tenants sharing infrastructure while maintaining:
- Data isolation (tenant A cannot see tenant B's data)
- Policy isolation (different thresholds per tenant)
- Execution isolation (one tenant's load doesn't affect another)

## Decision

**Multi-tenant isolation at data, policy, and execution boundaries.**

| Boundary | Mechanism |
|----------|-----------|
| Data | Tenant ID on all rows, row-level security |
| Policy | PolicyBundle scoped to tenant |
| Execution | Tile assignment, rate limiting |
| Graph | Subgraph partitioning by tenant |

## Consequences

### Benefits
- Single deployment serves multiple tenants
- Clear isolation boundaries
- Per-tenant customization

### Risks
- Noisy neighbor problems (mitigated by rate limiting)
- Complexity in cross-tenant operations (by design: not allowed)

## References

- ADR-014: Coherence Engine Architecture
