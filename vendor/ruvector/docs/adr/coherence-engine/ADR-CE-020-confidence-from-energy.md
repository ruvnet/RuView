# ADR-CE-020: Confidence from Energy

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

RuvLLM's `ConfidenceChecker` produces confidence scores, but:
- Scores are heuristic-based
- "Confidence" is often miscalibrated
- No mathematical grounding

Coherence energy provides a principled alternative.

## Decision

**Confidence scores derived from coherence energy with sigmoid mapping.**

Mapping:
```rust
fn confidence_from_energy(energy: f32, scale: f32, threshold: f32) -> f32 {
    // Low energy → high confidence
    // High energy → low confidence
    let scaled = scale * (energy - threshold);
    1.0 / (1.0 + scaled.exp())
}
```

Properties:
- Energy = 0 → Confidence ≈ 1.0 (perfectly coherent)
- Energy = threshold → Confidence = 0.5 (uncertain)
- Energy >> threshold → Confidence → 0 (incoherent)

## Consequences

### Benefits
- Confidence has mathematical grounding
- "I don't know" is provable (high energy)
- Calibration through energy scale tuning

### Risks
- Sigmoid parameters need tuning
- Different domains may need different mappings

## References

- ADR-014: Coherence Engine Architecture, "RuvLLM Integration"
- ADR-CE-013: Not Prediction
