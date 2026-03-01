# ADR-043: External Intelligence Providers for SONA Learning

| Field       | Value                                          |
|-------------|------------------------------------------------|
| Status      | Accepted                                       |
| Date        | 2025-02-21                                     |
| Authors     | @grparry (proposal), ruv (implementation)      |
| Supersedes  | —                                              |
| Origin      | PR #190 (renumbered from ADR-029 to avoid collision with ADR-029-rvf-canonical-format) |

## Context

RuvLLM's learning loops — SONA trajectory recording, HNSW embedding classification, and model router calibration — depend on quality signals to distinguish good executions from bad ones. Today, those signals come from ruvllm's own inference pipeline: a request completes, a quality score is computed internally, and the score feeds back into the learning loops.

This works when ruvllm is the entire system. But increasingly, ruvllm operates as one component within larger orchestration pipelines — workflow engines, CI/CD systems, coding assistants, multi-agent frameworks — where the *real* quality signal lives outside ruvllm. The external system knows whether the task actually met acceptance criteria, whether tests passed, whether the human reviewer approved or rejected the output. Ruvllm doesn't have access to any of that.

### The Gap

ADR-002 established Ruvector as the unified memory layer and defined the Witness Log schema with `quality_score: f32`. ADR-CE-021 established that multiple systems (RuvLLM, Prime-Radiant) can contribute trajectories to a shared SONA instance. But neither ADR addresses **how external systems feed quality data in**.

### Existing Extension Precedents

Ruvllm already has well-designed trait-based extension points:

| Trait | Purpose | Location |
|-------|---------|----------|
| `LlmBackend` | Pluggable inference backends | `crates/ruvllm/src/backends/mod.rs:756` |
| `Tokenizer` | Pluggable tokenization | Trait object behind `Option<&dyn Tokenizer>` |

An intelligence provider follows the same pattern — a trait that external integrations implement, registered with the intelligence loader at startup.

## Decision

**Option B — Trait-Based Intelligence Providers**, with a built-in file-based provider as the default implementation.

This gives the extensibility of a trait interface while keeping the simplicity of file-based exchange for the common case. Non-Rust systems write a JSON file; a built-in `FileSignalProvider` reads it. Rust-native integrations can implement the trait directly for tighter control.

## Architecture

```
IntelligenceLoader (new component in intelligence module)
├── register_provider(Box<dyn IntelligenceProvider>)
├── load_all_signals() -> Vec<QualitySignal>
│   ├── iterate registered providers
│   ├── call provider.load_signals()
│   └── merge with optional quality_weights()
└── Built-in: FileSignalProvider
    ├── reads JSON from .claude/intelligence/data/
    └── returns Vec<QualitySignal>
```

### Integration Points

| Component | How Signals Flow In |
|-----------|-------------------|
| SONA Instant Loop | `QualitySignal.quality_score` → trajectory quality |
| SONA Background Loop | Batch of signals → router training data |
| Embedding Classifier | `task_description` → embedding, `outcome` → label |
| Model Router | `calibration_bias()` on `TaskComplexityAnalyzer` |

### Key Types

```rust
pub struct QualitySignal {
    pub id: String,
    pub task_description: String,
    pub outcome: String,              // "success", "partial_success", "failure"
    pub quality_score: f32,           // 0.0 - 1.0
    pub human_verdict: Option<String>,
    pub quality_factors: Option<QualityFactors>,
    pub completed_at: String,         // ISO 8601
}

pub struct QualityFactors {
    pub acceptance_criteria_met: Option<f32>,
    pub tests_passing: Option<f32>,
    pub no_regressions: Option<f32>,
    pub lint_clean: Option<f32>,
    pub type_check_clean: Option<f32>,
    pub follows_patterns: Option<f32>,
    pub context_relevance: Option<f32>,
    pub reasoning_coherence: Option<f32>,
    pub execution_efficiency: Option<f32>,
}

pub trait IntelligenceProvider: Send + Sync {
    fn name(&self) -> &str;
    fn load_signals(&self) -> Result<Vec<QualitySignal>>;
    fn quality_weights(&self) -> Option<ProviderQualityWeights> { None }
}
```

## Design Constraints

- **Zero overhead when unused.** No providers registered = no behavior change.
- **File-based by default.** Simplest provider reads a JSON file — no network calls.
- **No automatic weight changes.** Providers supply signals; weight changes are human decisions.
- **Backward compatible.** Existing loading continues unchanged. Providers are additive.

## Existing Code References

| Item | Status | Location |
|------|--------|----------|
| `LlmBackend` trait | EXISTS | `crates/ruvllm/src/backends/mod.rs:756` |
| `record_feedback()` | EXISTS | `crates/ruvllm/src/claude_flow/model_router.rs:646` |
| `QualityWeights` (metrics) | EXISTS | `crates/ruvllm/src/quality/metrics.rs:262` |
| `IntelligenceProvider` trait | NEW | `crates/ruvllm/src/intelligence/mod.rs` |
| `FileSignalProvider` | NEW | `crates/ruvllm/src/intelligence/mod.rs` |
| `IntelligenceLoader` | NEW | `crates/ruvllm/src/intelligence/mod.rs` |
| `calibration_bias()` | NEW | `crates/ruvllm/src/claude_flow/model_router.rs` |

## Implementation

### Files Created

| # | Path | Description |
|---|------|-------------|
| 1 | `crates/ruvllm/src/intelligence/mod.rs` | IntelligenceProvider trait, QualitySignal, FileSignalProvider, IntelligenceLoader |
| 2 | `docs/adr/ADR-043-external-intelligence-providers.md` | This ADR |

### Files Modified

| # | Path | Changes |
|---|------|---------|
| 1 | `crates/ruvllm/src/lib.rs` | Add `pub mod intelligence;` + re-exports |
| 2 | `crates/ruvllm/src/claude_flow/model_router.rs` | Add `calibration_bias()` to TaskComplexityAnalyzer |

## Consequences

### Positive

1. **Clean integration boundary.** External systems implement one trait instead of modifying ruvllm internals.
2. **Follows established patterns.** Same approach as `LlmBackend` — familiar to anyone who has extended ruvllm.
3. **Language-agnostic in practice.** Non-Rust systems write JSON; `FileSignalProvider` reads it.
4. **Graceful when absent.** No providers = no behavior change. File missing = empty signal set.
5. **Testable.** Providers can be unit-tested independently.

### Negative

1. One more trait to maintain (small surface: 2 required methods, 1 optional).
2. Non-Rust systems must use the file path unless they write a Rust wrapper.

## Related Decisions

- **ADR-002**: RuvLLM Integration with Ruvector — Witness Log schema with `quality_score: f32`
- **ADR-029**: RVF Canonical Format — (the existing ADR-029, not to be confused with this one)
- **ADR-CE-021**: Shared SONA — multiple external systems contributing trajectories
- **ADR-004**: KV Cache Management — tiered, policy-driven approach benefiting from better calibration
