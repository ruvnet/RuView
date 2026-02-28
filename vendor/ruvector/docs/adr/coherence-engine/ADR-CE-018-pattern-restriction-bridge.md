# ADR-CE-018: Pattern-to-Restriction Bridge

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

RuvLLM's `ReasoningBank` stores successful patterns with verdicts. Prime-Radiant's restriction maps define constraints. These can reinforce each other:
- Successful patterns → what "coherence" looks like
- Failed patterns → what "incoherence" looks like

## Decision

**ReasoningBank patterns feed learned restriction map training.**

Bridge process:
```rust
impl PatternToRestrictionBridge {
    fn learn_from_verdict(&mut self, pattern_id: PatternId, verdict: Verdict) {
        if verdict.success_score > 0.8 {
            // Success: train ρ to produce zero residual
            self.restriction_maps[pattern_id]
                .train(source, target, zero_residual);
        } else {
            // Failure: train ρ to produce high residual
            self.restriction_maps[pattern_id]
                .train(source, target, failure_residual);
        }
    }
}
```

## Consequences

### Benefits
- Experience improves constraint accuracy
- Successful patterns define "good" coherence
- Failed patterns help detect future failures

### Risks
- Biased patterns lead to biased constraints
- Need sufficient positive and negative examples

## References

- ADR-014: Coherence Engine Architecture, "RuvLLM Integration"
- ruvllm/src/reasoning_bank/
