# ADR-CE-017: Unified Audit Trail

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

RuvLLM has `WitnessLog` for inference audit. Prime-Radiant has `WitnessRecord` for coherence decisions. Two separate audit trails create:
- Fragmented compliance story
- Difficult cross-referencing
- Duplicate storage

## Decision

**WitnessLog and Prime-Radiant governance share single audit trail.**

Unified structure:
```rust
pub struct UnifiedWitnessLog {
    coherence_witnesses: Vec<WitnessRecord>,
    inference_witnesses: WitnessLog,
}

pub struct GenerationWitness {
    inference: InferenceWitness,
    coherence: WitnessRecord,
    hash_chain: Hash,
}
```

Every LLM generation links:
- Inference witness (what was generated)
- Coherence witness (why it was allowed)
- Hash chain (tamper-evident ordering)

## Consequences

### Benefits
- Single audit trail for compliance
- Cross-reference inference ↔ coherence decisions
- Reduced storage (shared chain)

### Risks
- Migration from two systems to one
- Both systems must agree on witness format

## References

- ADR-014: Coherence Engine Architecture, "RuvLLM Integration"
- ADR-CE-005: First-Class Governance Objects
