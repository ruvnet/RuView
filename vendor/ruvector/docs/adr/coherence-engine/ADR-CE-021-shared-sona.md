# ADR-CE-021: Shared SONA

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

Both RuvLLM and Prime-Radiant use SONA for adaptive tuning:
- RuvLLM: Quality thresholds, routing weights
- Prime-Radiant: Coherence thresholds, escalation triggers

Running two SONA instances wastes resources and may learn conflicting adaptations.

## Decision

**SonaIntegration shared between ruvllm and Prime-Radiant.**

Shared components:
- `SonaEngine`: Single instance with multiple learning targets
- `ReasoningBank`: Unified pattern storage
- `EWC++`: Consolidated knowledge across both systems

Configuration:
```rust
pub struct SharedSona {
    engine: SonaEngine,
    llm_targets: Vec<LlmLearningTarget>,
    coherence_targets: Vec<CoherenceLearningTarget>,
}
```

Learning coordination:
- Both systems contribute trajectories
- EWC++ prevents forgetting across domains
- Patterns accessible to both systems

## Consequences

### Benefits
- Unified adaptation reduces resource usage
- Cross-domain learning (LLM patterns help coherence, vice versa)
- Consistent behavior across systems

### Risks
- Coupling between systems
- Bad learning in one domain affects both

## References

- ADR-014: Coherence Engine Architecture, "RuvLLM Integration"
- sona crate documentation
