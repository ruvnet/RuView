# ADR-CE-022: Failure Learning

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

RuvLLM's `ErrorPatternLearner` detects:
- Repeated error patterns
- Systematic failures
- Edge cases that cause problems

This knowledge should improve Prime-Radiant's detection.

## Decision

**ErrorPatternLearner updates restriction maps on failure detection.**

Process:
1. ErrorPatternLearner identifies failure pattern
2. Extract embeddings from failure context
3. Compute what residual "should have been" (high, since failure)
4. Train restriction map to produce high residual for similar inputs
5. Future similar inputs trigger coherence warning

Integration:
```rust
impl ErrorPatternLearner {
    fn on_error_pattern_detected(&self, pattern: ErrorPattern) {
        let bridge = self.restriction_bridge.lock();
        bridge.learn_failure_pattern(
            pattern.context_embedding,
            pattern.output_embedding,
            pattern.severity,
        );
    }
}
```

## Consequences

### Benefits
- System learns from mistakes
- Future similar failures detected proactively
- Restriction maps become smarter over time

### Risks
- False positive errors teach wrong constraints
- Need to distinguish systematic vs. random failures

## References

- ADR-014: Coherence Engine Architecture, "RuvLLM Integration"
- ADR-CE-018: Pattern-to-Restriction Bridge
- ruvllm/src/reflection/error_pattern.rs
