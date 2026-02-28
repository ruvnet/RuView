# ADR-CE-014: Reflex Lane Default

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

A coherence system that escalates too often becomes:
- Slow (every operation waits for heavy compute)
- Noisy (constant human escalations)
- Ignored (users bypass the system)

## Decision

**Reflex lane default - most updates stay low-latency, escalation only on sustained incoherence.**

Design principles:
1. **Default to Lane 0**: Most operations complete in <1ms
2. **Transient spikes tolerated**: Brief energy increases don't escalate
3. **Persistence triggers escalation**: Only sustained/growing incoherence moves up lanes
4. **Human lane is last resort**: Lane 3 only when automated systems cannot resolve

Persistence detection:
```rust
fn is_escalation_needed(history: &EnergyHistory, window: Duration) -> bool {
    history.is_above_threshold(threshold, window) ||
    history.is_trending_up(window)
}
```

## Consequences

### Benefits
- System stays responsive under normal operation
- Escalation is meaningful (not noise)
- Users trust the system (it's not crying wolf)

### Risks
- Might miss real problems that appear transient
- Persistence window requires tuning

## References

- ADR-014: Coherence Engine Architecture, Section 3
- ADR-CE-006: Compute Ladder
