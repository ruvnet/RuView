# ADR-CE-019: Memory as Nodes

**Status**: Accepted
**Date**: 2026-01-22
**Parent**: ADR-014 Coherence Engine Architecture

## Context

RuvLLM has three memory types:
- `AgenticMemory`: Long-term patterns
- `WorkingMemory`: Current context
- `EpisodicMemory`: Conversation history

These memories can contradict each other. Currently no systematic way to detect.

## Decision

**AgenticMemory, WorkingMemory, EpisodicMemory become sheaf nodes.**

Integration:
```rust
pub struct MemoryCoherenceLayer {
    agentic: AgenticMemory,
    working: WorkingMemory,
    episodic: EpisodicMemory,
    graph: SheafGraph,
}
```

When memory is added:
1. Create sheaf node with memory embedding
2. Add edges to related memories
3. Compute coherence energy
4. Alert if incoherent memory detected

Edge types:
- Temporal: Episode N should be consistent with N-1
- Semantic: Related facts should agree
- Hierarchical: Specific facts consistent with general patterns

## Consequences

### Benefits
- Detect contradictory memories before they cause problems
- Unified coherence across all memory types
- Can query "is my context self-consistent?"

### Risks
- Overhead for every memory write
- Edge creation requires semantic analysis

## References

- ADR-014: Coherence Engine Architecture, "RuvLLM Integration"
- ruvllm/src/context/
