# ADR-CE-015: Adapt Without Losing Control

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

Static systems become stale. Adaptive systems can drift or be gamed. The coherence engine needs to:
- Learn from experience
- Improve over time
- Maintain governance and control

## Decision

**Adapt without losing control - persistent tracking enables learning within governance.**

Adaptation mechanisms:
1. **Threshold autotuning**: SONA proposes, humans approve
2. **Learned restriction maps**: GNN training with EWC++ (no forgetting)
3. **ReasoningBank patterns**: Store successful approaches
4. **Deterministic replay**: Verify adaptations against history

Control mechanisms:
1. **Policy bundles require signatures**: No unauthorized changes
2. **Witness chain is immutable**: Cannot hide past decisions
3. **Lineage tracking**: Every adaptation has provenance
4. **Rollback support**: Can revert to previous policy

## Consequences

### Benefits
- System improves with experience
- Governance maintained throughout
- Can audit all adaptations

### Risks
- Adaptation speed limited by approval process
- Learning quality depends on trace quality

## References

- ADR-014: Coherence Engine Architecture
- ADR-CE-007: Threshold Autotuning
