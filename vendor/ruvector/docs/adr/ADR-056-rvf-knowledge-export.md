# ADR-056: RVF Knowledge Export for Developer Onboarding

**Status**: Accepted
**Date**: 2026-02-26
**Authors**: ruv.io, RuVector Architecture Team
**Deciders**: Architecture Review Board
**SDK**: Claude-Flow

## Context

### The Onboarding Problem

The RuVector project has accumulated 3,135 commits across 99 days (2025-11-19 to 2026-02-26), producing 91 crates, 55+ ADRs, and a sophisticated RVF format specification. New developers face a steep learning curve:

1. **No single entry point** — Knowledge is scattered across ADRs, commit messages, code comments, and claude-flow memory
2. **Implicit architecture** — Many design decisions live in commit history, not documentation
3. **Format complexity** — RVF has 25 segment types, 5 domain profiles, and integrations with 7+ libraries
4. **Computation depth** — 85+ crates covering GNN, graph transformers, solvers, LLM inference, quantum simulation, formal verification

### The RVF Opportunity

RVF (ADR-029) already defines a self-describing binary format with META_SEG for key-value metadata, WITNESS_SEG for audit trails, and the `rvf-adapter-claude-flow` crate for memory persistence. A knowledge export in RVF format serves as both:

1. **A practical onboarding artifact** — Everything a developer needs to understand RuVector
2. **A live demonstration** — The export itself exercises the RVF format, proving the format works

## Decision

### Export all accumulated project knowledge as an RVF-backed knowledge base

The export lives at `docs/research/knowledge-export/` and consists of:

1. **`ruvector-knowledge.rvf.json`** — Structured knowledge base in JSON (human-readable RVF manifest representation)
2. **`QUICKSTART.md`** — Developer onboarding guide distilled from the knowledge base
3. **This ADR** — Governance record for the export process

### Knowledge Segments

The export maps project knowledge to RVF segment types:

| RVF Segment | Knowledge Category | Content |
|-------------|-------------------|---------|
| META_SEG (0x07) | Project Identity | Name, version, license, repo, timeline, statistics |
| PROFILE_SEG (0x0B) | Architecture Profiles | Crate taxonomy, module purposes, feature flags |
| WITNESS_SEG (0x0A) | Decision History | All ADRs summarized with status and rationale |
| INDEX_SEG (0x02) | Dependency Graph | Inter-crate dependency map for navigation |
| OVERLAY_SEG (0x03) | Evolution Timeline | Major milestones and architectural shifts |
| SKETCH_SEG (0x09) | Patterns & Conventions | Coding patterns, testing strategy, CI/CD practices |
| JOURNAL_SEG (0x04) | Lessons Learned | Debugging insights, security findings, performance discoveries |

### Who Uses This

| Audience | Use Case |
|----------|----------|
| New developers | Read QUICKSTART.md, browse knowledge base for architecture overview |
| AI agents | Load knowledge base as context for code generation and review |
| Contributors | Understand design decisions before proposing changes |
| Downstream users | Evaluate RuVector capabilities and integration points |

## Consequences

### Benefits

1. **Single-file onboarding** — One JSON file contains the entire project knowledge graph
2. **RVF dogfooding** — Proves the format's metadata and witness capabilities
3. **AI-consumable** — Structured format that LLMs can parse and reason over
4. **Version-controlled** — Ships with the repo, stays synchronized

### Risks

| Risk | Mitigation |
|------|------------|
| Knowledge becomes stale | Export script can be re-run; ADR mandates updates at major versions |
| Export is too large | Structured by segment type; consumers can load specific sections |
| Sensitive data leaks | Export draws only from public repo content, never from .env or credentials |

## Related Decisions

- **ADR-029**: RVF canonical format (defines the segment model used here)
- **ADR-030**: Cognitive containers (export is a lightweight cognitive container)
- **ADR-031**: RVF example repository (this export serves as a living example)
