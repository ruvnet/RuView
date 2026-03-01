# ADR-CE-007: Thresholds Auto-Tuned from Production Traces

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

Fixed thresholds become stale as:
- System behavior evolves
- New edge types are added
- Domain characteristics change

Manual tuning is expensive and error-prone.

## Decision

**Thresholds auto-tuned from production traces with governance approval.**

Process:
1. **Collect traces**: Energy values, gate decisions, outcomes
2. **Analyze**: SONA identifies optimal threshold candidates
3. **Propose**: System generates new PolicyBundle with updated thresholds
4. **Approve**: Required approvers sign the bundle
5. **Deploy**: New thresholds become active

Constraints:
- Auto-tuning proposes, humans approve
- Changes tracked in audit log
- Rollback supported via new PolicyBundle

## Consequences

### Benefits
- Thresholds adapt to changing conditions
- Governance maintained (human approval required)
- Historical analysis enables data-driven decisions

### Risks
- Bad traces lead to bad proposals
- Approval bottleneck if too many proposals

## References

- ADR-014: Coherence Engine Architecture, Section 6
- ADR-CE-015: Adapt Without Losing Control
