# ADR-036: RuVector AGI Cognitive Container with Claude Code Orchestration

**Status**: Partially Implemented
**Date**: 2026-02-15 (updated 2026-02-17)
**Decision owners**: RuVector platform team, Claude Flow orchestration team, RVF runtime team
**Depends on**: ADR-029 (RVF Canonical Format), ADR-030 (Cognitive Container), ADR-033 (Progressive Indexing Hardening), ADR-034 (QR Cognitive Seed), ADR-035 (Capability Report), ADR-039 (RVF Solver WASM AGI Integration)
**Affects**: `rvf-types/src/agi_container.rs`, `rvf-runtime`, `npm/packages/rvf-solver/`, `npm/packages/rvf/`

## Context

A state change into general intelligence can emerge when two conditions hold:

1. **Existential facilities** -- a substrate that can persist identity, memory,
   constraints, health signals, and self-maintenance.
2. **Architectural organization** -- a framework that can package the system,
   control execution, and enforce repeatability while enabling incremental
   self-reinforced feedback loops.

RuVector is the existential substrate. RVF is the organizational and packaging
framework. Claude Code is the runtime orchestrator for planning and execution,
using agent teams and tool connectivity via MCP.

The deliverable is a portable intelligence package that other teams can run and
obtain the same graded outcomes, with replayable witness logs, policy controls,
and deterministic environment capture.

## Problem Statement

We need an architecture that can do all of the following in one system:

1. Learn continuously from real-world event streams
2. Maintain its own structural health and recover from corruption or drift
3. Act through tools with governed authority
4. Produce repeatable outcomes across machines and teams
5. Package the full intelligence state so it can be shipped, audited, and replayed

Most LLM-centered architectures measure success by static accuracy, but this
thesis needs longitudinal coherence under mutation. This ADR defines that
system boundary explicitly.

## Decision Drivers

1. Repeatable outcomes, not just plausible responses
2. Long-horizon coherence under continuous updates
3. Governance by default, including proof trails for actions
4. Minimal reliance on hidden model internals for learning
5. Portability across environments, including edge and offline modes
6. Strong separation of control plane and data plane
7. Tool-use reliability, batching, and reduced context pollution

Claude Code is chosen as orchestrator because it is designed to read codebases,
edit files, run commands, manage workflows, and integrate with external systems
via MCP, including multi-agent teams coordinated by a lead.

Programmatic tool calling is used as the preferred high-reliability tool
orchestration strategy because it makes control flow explicit in code and
reduces repeated model round-trips and context bloat.

## Definitions

| Term | Definition |
|------|-----------|
| **RuVector substrate** | Persistent world model combining vectors, graphs, constraints, and signals. Supports graph querying via Cypher. Includes self-learning and graph neural embedding updates, with dynamic minimum-cut as a coherence signal. |
| **RVF framework** | Cognitive container format that packages data, indexes, models, policies, and runtime into a single artifact. A single file that stores vectors and models, boots as a Linux microservice, accelerates queries using eBPF, branches at cluster granularity, and provides cryptographic witness chains. |
| **Claude Code orchestrator** | Agentic coding and task execution environment that runs in terminal, IDE, desktop, and web. Connects external tools via MCP. Coordinates agent teams. |
| **Claude Flow** | Multi-agent orchestration layer that turns Claude Code into a swarm-style coordinator with router, agents, shared memory, and learning loop. |
| **Structural health** | Measurable invariants indicating world model integrity: coherence gates, contradiction tracking, memory integrity, policy compliance, rollback readiness. |
| **Witness chain** | Cryptographic attestation trail linking each state change to inputs, decisions, and tool outputs. See ADR-035. |
| **Same results** | Identical graded outcomes and artifacts for a benchmark run, not necessarily identical intermediate tokens. Enforced through replay mode and verification mode. |

## Considered Options

| # | Option | Verdict |
|---|--------|---------|
| 1 | LLM-only agent with prompt history and ad-hoc logs | Rejected: no structural health, no reversibility, no packaging |
| 2 | LLM + vector retrieval memory only | Rejected: no coherence gating, no witness chains, no portable replay |
| 3 | LLM + RuVector world model + RVF cognitive container, orchestrated by Claude Code and Claude Flow | **Selected** |

**Rationale**: Options 1 and 2 cannot meet the thesis because they lack explicit
structural health machinery, reversible state transitions, and portable
replayable intelligence packaging.

## Decision

Build the AGI system as a closed-loop cognitive container:

1. **Claude Code** is the control plane orchestrator. It spawns an agent team
   and coordinates planning and execution.
2. **Claude Flow** provides the swarm orchestration model, routing tasks to
   specialized agents and managing shared memory and learning loop semantics.
3. **RuVector** is the existential substrate, storing world model state, typed
   memory, constraints, and coherence signals, queryable via graph queries
   and vector search.
4. **RVF** is the portable intelligence package format. It encapsulates the
   agent runtime, RuVector state snapshot and deltas, policies, indexes, tool
   adapters, and the evaluation harness so others can reproduce the same
   graded results.
5. **Learning** occurs primarily by structured memory mutation and skill
   promotion governed by coherence and evaluation gates, not by continuous
   weight updates.

## Architecture Overview

### System Boundary

**Inside the boundary:**
1. Claude Code lead session
2. Claude Flow router and swarm manager
3. Tool adapters and execution sandbox
4. RuVector database cluster (or embedded instance)
5. RVF container runtime and witness chain engine (ADR-035)
6. Evaluation harness and graders

**Outside the boundary:**
1. External data sources (repos, ticketing, logs, sensors)
2. External model provider infrastructure
3. Human approvals (if policy requires)

### High-Level Data Flow

```
Event Ingestion ──> World Model Update Proposal
       │                      │
       │               Structural Health Gate
       │                      │
       │              ┌───────┴───────┐
       │              │  Gate PASS?   │
       │              │  yes    no    │
       │              │   │     │     │
       │              │  Commit Reject│
       │              │   │     │     │
       │              └───┴─────┘     │
       │                  │           │
       ▼                  ▼           ▼
 Plan & Act Loop    Reflection    Rollback &
 (Claude Code +     & Compress    Quarantine
  Claude Flow)          │
       │                │
       ▼                ▼
 Commit & Witness (RVF ADR-035)
```

1. **Event ingestion**: Real-world events arrive and are normalized into a
   canonical event schema.
2. **World model update proposal**: The system proposes graph mutations and
   memory writes in RuVector.
3. **Structural health gating**: Coherence checks, contradiction checks, and
   policy checks determine if the proposal can be committed.
4. **Plan and act loop**: Claude Code and Claude Flow coordinate tool calls to
   act in the environment, using programmatic tool calling patterns.
5. **Reflection and compression**: Results are summarized into stable facts,
   procedures, and counterexamples.
6. **Commit and witness**: Deltas are committed into RuVector and sealed into
   the RVF witness chain (ADR-035).

### Control Plane / Data Plane Separation

| Aspect | Control Plane | Data Plane |
|--------|--------------|------------|
| **Who** | Claude Code + Claude Flow | RuVector + RVF |
| **Does** | Decides what to do; generates proposed deltas and tool actions | Executes storage, retrieval, graph ops, embeddings, coherence |
| **Varies** | Internal reasoning may vary between runs | Only gated commits become reality |
| **Enforces** | Plans and policies | Packaging, execution boundaries, attestations |

This separation is the core of repeatability.

## Components and Responsibilities

### Component A: Claude Code Lead Agent

| Inputs | Outputs |
|--------|---------|
| Task description | Plans |
| Current RVF container identity and policy | Tool calls |
| RuVector retrieval results | Proposed memory mutations |
| Tool outputs and environment observations | Commit requests |

Key capabilities: agent teams for parallel decomposition, MCP tool connectivity,
project instruction loading for consistent behavior across runs.

### Component B: Claude Flow Swarm Manager

| Inputs | Outputs |
|--------|---------|
| Lead agent goal graph | Sub-agent tasks |
| System policy limits | Consensus proposals |
| RuVector shared memory state | Aggregated plan; learning loop updates |

Architecture: router-to-swarm-to-agents with learning loop and shared memory.

### Component C: RuVector Substrate

| Inputs | Outputs |
|--------|---------|
| Events, text, code, images, structured records | Retrieved memories and facts |
| Embeddings, graph mutation deltas | Graph query results (Cypher) |
| Health telemetry updates | Embedding/ranking updates (self-learning) |
| | Coherence signals (dynamic minimum-cut) |

### Component D: RVF Cognitive Container Runtime

| Inputs | Outputs |
|--------|---------|
| Container manifest | Bootable runtime environment |
| Segmented data blobs | Reproducible execution environment |
| Policy and permissions | Signed witness records (ADR-035) |
| Cryptographic keys | Branchable snapshots |

### Component E: Tool Execution Sandbox

| Inputs | Outputs |
|--------|---------|
| Tool call plans from Claude Code | Tool results as structured objects |
| Programmatic tool calling scripts | Tool receipts with hashes |
| Policy rules | Failure modes and retry classifications |

## RuVector World Model Schema

### Node Types

| # | Type | Purpose |
|---|------|---------|
| 1 | **AgentIdentity** | Stable identity, keys, role, authority limits |
| 2 | **Event** | Normalized external observation (timestamp, source, payload hash) |
| 3 | **Claim** | Statement that may be true or false, linked to evidence |
| 4 | **Evidence** | Pointer to tool output, document excerpt, test output, sensor observation |
| 5 | **Plan** | Goal tree, constraints, success criteria, expected cost |
| 6 | **Action** | Tool invocation request with preconditions and expected effect |
| 7 | **Outcome** | Observed effects, pass/fail, test results, diffs, side effects |
| 8 | **Skill** | Reusable procedure with applicability conditions, constraints, and tests |
| 9 | **Policy** | Rules for permissions and safety boundaries |
| 10 | **HealthSignal** | Coherence metrics, drift, contradiction density, memory integrity |

### Edge Types

| Edge | Semantics |
|------|----------|
| `CAUSED` | Event CAUSED Claim or Outcome |
| `SUPPORTS` | Evidence SUPPORTS Claim |
| `CONTRADICTS` | Claim CONTRADICTS Claim |
| `DEPENDS_ON` | Plan DEPENDS_ON Skill or Evidence |
| `EXECUTES` | Action EXECUTES Tool |
| `PRODUCED` | Action PRODUCED Outcome |
| `PROMOTED_FROM` | Skill PROMOTED_FROM repeated successful Plans |
| `BLOCKED_BY` | Action BLOCKED_BY Policy |
| `HEALTH_OF` | HealthSignal HEALTH_OF subsystem or memory region |

### Invariants

| # | Invariant | Rule |
|---|-----------|------|
| 1 | **Evidence binding** | Any externally testable claim must have at least one Evidence edge; otherwise tagged `unverified` and cannot justify irreversible actions |
| 2 | **Contradiction locality** | A contradiction edge must reference the minimal conflicting claims, not a broad document blob |
| 3 | **Action gating** | Any action that changes external state must reference the policy decision node that allowed it |
| 4 | **Replay completeness** | Every tool output referenced by evidence must be hashable and stored or re-derivable from deterministic inputs |

## Structural Health and Coherence Gate Design

This is the mechanism that operationalizes the state-change thesis. It turns
continuous learning into safe incremental commits.

### Health Signals

| # | Signal | Computation |
|---|--------|-------------|
| 1 | **Coherence score** | Dynamic minimum-cut on active working set subgraph. Measures separability between consistent clusters and contradiction boundaries. |
| 2 | **Contradiction pressure** | Rate of new contradiction edges per unit time, weighted by claim criticality |
| 3 | **Memory integrity** | Schema validation success, witness chain continuity, segment hash integrity |
| 4 | **Tool reliability** | Error rates, retries, timeouts, drift in tool schemas |
| 5 | **Cost stability** | Cost-per-solved-task trend, abnormal spikes |

### Coherence Gate Rules

| Rule | Trigger | Action |
|------|---------|--------|
| 1. Block unsafe commits | Coherence score drops below threshold after proposed delta | Reject and open repair plan |
| 2. Require counterexample storage | An outcome fails | Counterexample must be created and linked before any new skill promotion |
| 3. Limit graph churn | Contradiction pressure exceeds threshold | Freeze new skill promotion; focus on repair and consolidation |
| 4. Quarantine volatile memories | New claims arrive | Enter volatile pool until reinforced by independent evidence or repeated success |

## Learning Loop Design

### Learning Primitives

1. **Episodic capture**: Store event, plan, action, outcome chain as an episode
2. **Reflection**: Extract stable claims and failure causes, bind evidence
3. **Consolidation**: Merge redundant claims, compress long traces into summaries
   plus pointers, maintain witness chain
4. **Skill promotion**: Promote procedure into Skill node only when criteria met

### Skill Promotion Criteria

A candidate becomes a skill when **all** of the following are true:

1. It has succeeded K times on non-identical inputs
2. It has at least one negative example recorded and bounded
3. It has objective graders that validate outputs
4. It does not increase policy violations or coherence degradation

### Self-Reinforced Feedback Loops

A loop is self-reinforced when successful actions increase the system's future
probability of selecting high-value plans, while structural health remains
within bounds.

**Mechanism:**
- Success produces evidence and updated skill priors
- RuVector retrieval makes these skills easier to select
- Coherence gates prevent runaway self-confirmation

## Repeatability and Portable Intelligence Packaging

### RVF Packaging Decision

One RVF artifact contains:

| Segment | Contents |
|---------|----------|
| **Manifest and identity** | Container ID, build ID, model routing config, policy version, tool adapter registry |
| **Runtime** | Claude Flow orchestrator config, agent role prompts, tool schemas, sandbox config |
| **RuVector snapshot** | Base world model graph, indexes, embeddings, skill library, policy nodes |
| **Delta journal** | Append-only commits with witness chain records (ADR-035) |
| **Evaluation harness** | Task suite, graders, scoring rules, replay scripts |

### Two Execution Modes

| Mode | Goal | Method | Pass Condition |
|------|------|--------|----------------|
| **Replay** | Bit-identical artifact reproduction | No external tool calls; use stored receipts and outputs | All graders match exactly; witness chain matches |
| **Verify** | Same graded outcomes under live tools | Tools called live; outputs stored and hashed | Outputs pass same tests; costs within expected bounds |

This is how you claim "same results" without over-promising identical token
sequences across different infrastructure.

### Determinism Controls

1. Pin model ID to a specific version in the container manifest
2. Set sampling for maximum determinism in production runs
3. Store prompt and instruction hashes for each run
4. Virtualize time for tasks that depend on timestamps
5. Freeze external dependencies by snapshotting repos and data sources
6. Record all tool outputs with hashes and schema versions

## AGI npm Package Distribution

The AGI capabilities of the cognitive container are distributed as npm packages,
enabling JavaScript/TypeScript consumers to access the self-learning engine,
witness chains, and HNSW index operations without a Rust toolchain.

### Package Ecosystem

| Package | Version | AGI Capabilities |
|---------|---------|-----------------|
| `@ruvector/rvf-solver` | 0.1.0 | Thompson Sampling PolicyKernel, KnowledgeCompiler, three-loop adaptive solver, SHAKE-256 witness chains, 18 context buckets, speculative dual-path execution |
| `@ruvector/rvf-node` | 0.1.6 | HNSW index statistics, witness chain verification, store freeze (snapshot), distance metric introspection |
| `@ruvector/rvf-wasm` | 0.1.5 | Witness chain verification (`rvf_witness_verify`), WASM microkernel for browser/edge |
| `@ruvector/rvf` | 0.1.8 | Unified SDK re-exporting all of the above; single `npm install` for full AGI access |

### Self-Learning Solver API

```typescript
import { RvfSolver } from '@ruvector/rvf';

const solver = await RvfSolver.create();

// Three-loop training: fast (solve) / medium (policy) / slow (compiler)
const result = solver.train({ count: 1000, minDifficulty: 1, maxDifficulty: 10 });

// Full acceptance test with A/B/C ablation modes
const manifest = solver.acceptance({ cycles: 5, holdoutSize: 100 });

// Inspect learned policy state
const policy = solver.policy();

// Export tamper-evident witness chain (73 bytes per entry)
const chain = solver.witnessChain();

solver.destroy();
```

### AGI NAPI Methods

The native Node.js bindings expose AGI-relevant operations:

| Method | Returns | AGI Purpose |
|--------|---------|-------------|
| `indexStats()` | `RvfIndexStats` | Introspect HNSW graph structure (layers, M, ef_construction) for coherence monitoring |
| `verifyWitness()` | `RvfWitnessResult` | Validate witness chain integrity for replay/verify modes |
| `freeze()` | `void` | Snapshot-freeze state for deterministic branching |
| `metric()` | `string` | Distance metric introspection for coherence signal computation |

### Integration with Cognitive Container

The npm packages map to cognitive container components:

| Container Component | npm Package | Segment |
|--------------------|-------------|---------|
| Self-learning engine | `@ruvector/rvf-solver` | SOLVER_SEG (computed in WASM) |
| Witness chain attestation | `@ruvector/rvf-solver` + `@ruvector/rvf-wasm` | WITNESS_SEG (0x0A) |
| Vector storage & retrieval | `@ruvector/rvf-node` | VEC_SEG, INDEX_SEG |
| HNSW index inspection | `@ruvector/rvf-node` | INDEX_SEG |
| Browser-side verification | `@ruvector/rvf-wasm` | WITNESS_SEG verification |

## MCP Tools

Core MCP tools to implement:

| Tool | Purpose |
|------|---------|
| `ruvector_query` | Vector search and filtered retrieval |
| `ruvector_cypher` | Graph query and traversal for claims, evidence, contradictions |
| `ruvector_commit_delta` | Propose and commit world model deltas behind coherence gates |
| `rvf_snapshot` | Create a branchable snapshot for experiments |
| `rvf_witness_export` | Export witness chain proofs for audit (ADR-035) |
| `rvf_solver_train` | Run self-learning solver training via `@ruvector/rvf-solver` |
| `rvf_solver_acceptance` | Execute full A/B/C ablation acceptance test |
| `eval_run` | Run the container's benchmark suite and return graded results |

## Security Model

### Threat Model

1. Prompt injection via untrusted content
2. Tool abuse and unintended side effects
3. Data exfiltration via tool channels
4. Memory poisoning causing long-horizon drift
5. Supply chain drift causing irreproducible results

### Controls

| # | Control | Mechanism |
|---|---------|-----------|
| 1 | Capability-based permissions | Each tool call requires explicit capability grants; high-risk actions require approvals |
| 2 | Policy as data | Policies live in RuVector and are embedded in RVF manifest; policy cannot silently change between runs |
| 3 | Witnessed commits | Every commit is attested with inputs, policy decision, and tool receipts (ADR-035) |
| 4 | Quarantine zone | Untrusted inputs enter quarantine; cannot directly affect skill promotion |
| 5 | Sandboxed execution | Tool scripts run in restricted environments; programmatic tool calling makes control flow explicit |

## Observability and Benchmarking

### Required Metrics

1. Success rate on task suite
2. Policy violations count
3. External side effects count
4. Contradiction rate
5. Coherence score trend
6. Rollback frequency and success
7. Dollars per solved task
8. p50 and p95 latency per task
9. Tool error rate

### Benchmark Tiers

| Tier | Name | Purpose |
|------|------|---------|
| 1 | Deterministic replay suite | Verifies packaging and witness integrity |
| 2 | Tool and memory suite | Measures long-horizon stability and coherence gating |
| 3 | Production domain suite | Measures real outcomes (repo issue fixes, compliance, deployments) |

### Proof Artifact per Run

Each run exports:
1. Run manifest
2. Task inputs and snapshots
3. All tool receipts and hashes
4. All committed deltas
5. Witness chain export (ADR-035)
6. Grader outputs and final scorecard

## Consequences

### Positive

1. **Clear system boundary** for intelligence measurement -- the composite
   system is evaluated, not the model in isolation
2. **Repeatability as a product feature** -- RVF container + witness chain +
   replay mode enables credible external validation
3. **Safety is structural** -- policies and coherence gates are part of the
   substrate, not an afterthought
4. **Multi-agent scalability** -- Claude Code agent teams + Claude Flow swarm
   routing supports parallel work and specialization

### Negative / Risks

1. **Complexity risk** -- system of systems; requires investment in harnesses
   and invariants early
2. **Non-determinism risk** from model providers -- replay mode mitigates by
   recording outputs
3. **Memory poisoning risk** -- powerful memory can amplify wrong beliefs if
   coherence gates are weak; bias toward evidence binding and counterexample capture
4. **Benchmark gaming risk** -- weak graders will be exploited; build robust
   graders first

## Implementation Plan

### Phase 1: Foundation

**Deliverables:**
1. RuVector schema and APIs for events, claims, evidence, contradictions
2. RVF container manifest format for model, policy, tool registry, snapshots
3. MCP server exposing RuVector and RVF operations to Claude Code
4. Basic witness log and delta commit pipeline (ADR-035 -- done)

**Exit criteria:** Replay mode works on a small deterministic suite.

### Phase 2: Coherence Gating

**Deliverables:**
1. Structural health signals and thresholds
2. Dynamic minimum-cut coherence metric integration
3. Rollback and quarantine semantics
4. Contradiction detection routines

**Exit criteria:** No irreversible external tool calls allowed when coherence is
below threshold.

### Phase 3: Learning and Skill Promotion

**Deliverables:**
1. Skill nodes, promotion criteria, and tests
2. Consolidation and compaction routines
3. Counterexample-driven repair

**Exit criteria:** Skills improve success rate over time without increasing
contradictions.

### Phase 4: Portable Intelligence Distribution

**Deliverables:**
1. One-RVF-file distribution pipeline
2. Public evaluation harness packaged inside RVF
3. Verification mode that produces same graded outcomes across machines

**Exit criteria:** Two independent teams run the same RVF artifact and achieve
the same benchmark scorecard.

## Resolved Design Questions

### Q1: First domain for proving the state-change thesis

**Decision: Repo automation** (software engineering lifecycle).

Rationale: This domain provides the strongest combination of (a) verifiable
outcomes (tests pass, code compiles, PR merges), (b) tool-rich environment
(git, CI, code editors via Claude Code), (c) naturally occurring event streams
(issues, commits, reviews), and (d) existing infrastructure in Claude Code +
Claude Flow. The evaluation harness measures: issues solved, test success rate,
regression introduction rate, cost per solved issue, and witness chain
completeness.

Subsequent domains (incident triage, governance workflows, edge autonomy)
are pursued after the repo automation scorecard achieves >= 60/100 solved
with zero policy violations.

### Q2: Authority levels

**Decision: Four-level authority model, default ReadMemory.**

```rust
#[repr(u8)]
pub enum AuthorityLevel {
    /// Read-only: query vectors, graphs, memories. No mutations.
    ReadOnly = 0,
    /// Write to internal memory: commit world model deltas behind
    /// coherence gates. No external tool calls.
    WriteMemory = 1,
    /// Execute tools: run sandboxed tools (file read/write, tests,
    /// code generation). External side effects are gated by policy.
    ExecuteTools = 2,
    /// Write external: push code, create PRs, send messages, modify
    /// infrastructure. Requires explicit policy grant per action class.
    WriteExternal = 3,
}
```

Each action in the world model must reference a policy decision node
(invariant #3, "Action gating") that grants at least the required authority
level. The container manifest declares the maximum authority level permitted
for a given execution. Higher levels require explicit policy override.

Default for Replay mode: `ReadOnly`.
Default for Verify mode: `ExecuteTools`.
Default for Live mode: `WriteMemory` (escalation to higher levels requires
policy grant per action class).

### Q3: Resource budgets

**Decision: Per-task resource budgets with hard caps.**

Every task execution is bounded by:

| Resource | Default Cap | Override |
|----------|-------------|---------|
| Wall-clock time per task | 300 seconds | Policy override, max 3600s |
| Total model tokens per task | 200,000 | Policy override, max 1,000,000 |
| Total cost per task | $1.00 | Policy override, max $10.00 |
| Tool calls per task | 50 | Policy override, max 500 |
| External write actions per task | 0 (ReadOnly) | Requires WriteExternal authority |

Budget exhaustion triggers graceful degradation: the task enters `Skipped`
outcome with a `BudgetExhausted` postmortem in the witness bundle.

### Q4: Coherence thresholds

**Decision: Three configurable thresholds stored in the container header.**

| Threshold | Default | Effect when breached |
|-----------|---------|---------------------|
| `min_coherence_score` | 0.70 | Block all commits; enter repair mode |
| `max_contradiction_rate` | 5.0 per 100 events | Freeze skill promotion |
| `max_rollback_ratio` | 0.20 | Halt Live execution; require human review |

These map to ADR-033's quality framework: the coherence score is analogous
to `ResponseQuality` -- it signals whether the system's internal state is
trustworthy enough to act on.

## Wire Format

### AgiContainerHeader (64 bytes, `repr(C)`)

The AGI container is stored as a `Meta` segment (`SegmentType::Meta = 0x07`)
in the RVF file, alongside the KERNEL_SEG, WASM_SEG, VEC_SEG, INDEX_SEG,
WITNESS_SEG, and CRYPTO_SEG that hold the actual payload data.

```
Offset  Type        Field               Description
------  ----        -----               -----------
0x00    u32         magic               0x52564147 ("RVAG")
0x04    u16         version             Header format version (currently 1)
0x06    u16         flags               Bitfield (see below)
0x08    [u8; 16]    container_id        Unique container UUID
0x18    [u8; 16]    build_id            Build UUID (changes on repackaging)
0x28    u64         created_ns          Creation timestamp (nanos since epoch)
0x30    [u8; 8]     model_id_hash       SHA-256 of pinned model ID, truncated
0x38    [u8; 8]     policy_hash         SHA-256 of governance policy, truncated
```

### Flags (u16 bitfield)

```
Bit   Name                    Description
---   ----                    -----------
0     AGI_HAS_KERNEL          KERNEL_SEG with micro Linux kernel present
1     AGI_HAS_WASM            WASM_SEG modules present
2     AGI_HAS_ORCHESTRATOR    Claude Code + Claude Flow config present
3     AGI_HAS_WORLD_MODEL     VEC_SEG + INDEX_SEG world model data present
4     AGI_HAS_EVAL            Evaluation harness (tasks + graders) present
5     AGI_HAS_SKILLS          Promoted skill library present
6     AGI_HAS_WITNESS         ADR-035 witness chain present
7     AGI_SIGNED              Container is cryptographically signed
8     AGI_REPLAY_CAPABLE      All tool outputs stored; supports replay mode
9     AGI_OFFLINE_CAPABLE     Container can run without network access
10    AGI_HAS_TOOLS           MCP tool adapter registry present
11    AGI_HAS_COHERENCE_GATES Coherence gate configuration present
```

### TLV Manifest Tags

Following the header, a TLV (tag-length-value) manifest contains the
container's configuration sections:

| Tag    | Name                   | Content |
|--------|------------------------|---------|
| 0x0100 | CONTAINER_ID           | Container UUID |
| 0x0101 | BUILD_ID               | Build UUID |
| 0x0102 | MODEL_ID               | Pinned model identifier (UTF-8) |
| 0x0103 | POLICY                 | Serialized governance policy |
| 0x0104 | ORCHESTRATOR           | Claude Code + Claude Flow config |
| 0x0105 | TOOL_REGISTRY          | MCP tool adapter registry |
| 0x0106 | AGENT_PROMPTS          | Agent role prompts |
| 0x0107 | EVAL_TASKS             | Evaluation task suite |
| 0x0108 | EVAL_GRADERS           | Grading rules |
| 0x0109 | SKILL_LIBRARY          | Promoted skill library |
| 0x010A | REPLAY_SCRIPT          | Replay automation script |
| 0x010B | KERNEL_CONFIG          | Kernel boot parameters |
| 0x010C | NETWORK_CONFIG         | Network configuration |
| 0x010D | COHERENCE_CONFIG       | Coherence gate thresholds and rules |
| 0x010E | PROJECT_INSTRUCTIONS   | Claude.md project instructions |
| 0x010F | DEPENDENCY_SNAPSHOT    | Dependency snapshot hashes |
| 0x0110 | AUTHORITY_CONFIG       | Authority level and resource budgets |
| 0x0111 | DOMAIN_PROFILE         | Target domain profile (RVText, etc.) |

Unknown tags are ignored (forward-compatible).

### Implementation

Types are fully implemented in `rvf-types/src/agi_container.rs` (972 lines, 24 tests).

**Implemented types:**

| Type | Size / Kind | Description | Tests |
|------|-------------|-------------|-------|
| `AgiContainerHeader` | 64 bytes (`repr(C)`) | Wire-format header with magic "RVAG" (0x52564147), `to_bytes()`/`from_bytes()` serialization, compile-time size assertion | 4 |
| `ExecutionMode` | `u8` enum | Replay (0), Verify (1), Live (2) with `TryFrom<u8>` | 1 |
| `AuthorityLevel` | `u8` enum | ReadOnly (0), WriteMemory (1), ExecuteTools (2), WriteExternal (3) with `TryFrom<u8>`, `PartialOrd`/`Ord`, `permits()`, `default_for_mode()` | 4 |
| `ResourceBudget` | struct | Per-task resource caps with `DEFAULT`, `EXTENDED`, `MAX` presets and `clamped()` method | 3 |
| `CoherenceThresholds` | struct | Three configurable thresholds (`min_coherence_score`, `max_contradiction_rate`, `max_rollback_ratio`) with `DEFAULT`, `STRICT` presets and `validate()` method | 5 |
| `ContainerSegments` | struct | Segment presence tracker with `validate(mode)` and `to_flags()` | 7 |
| `ContainerError` | enum | 6 variants: MissingSegment, TooLarge, InvalidConfig, SignatureInvalid, InsufficientAuthority, BudgetExhausted with `Display` | 1 |

**Constants defined:**
- 13 flag constants (`AGI_HAS_KERNEL` through `AGI_HAS_DOMAIN_EXPANSION`, bits 0-12)
- 22 TLV manifest tag constants (`AGI_TAG_CONTAINER_ID` 0x0100 through `AGI_TAG_COUNTEREXAMPLES` 0x0115)
- Includes 4 domain expansion tags: `AGI_TAG_TRANSFER_PRIOR` (0x0112), `AGI_TAG_POLICY_KERNEL` (0x0113), `AGI_TAG_COST_CURVE` (0x0114), `AGI_TAG_COUNTEREXAMPLES` (0x0115)

**Key design properties:**
- `AuthorityLevel::permits()` enables level comparison: `WriteExternal` permits all lower levels
- `AuthorityLevel::default_for_mode()` maps Replay->ReadOnly, Verify->ExecuteTools, Live->WriteMemory
- `ResourceBudget::clamped()` enforces hard ceilings (`MAX` preset) that cannot be overridden
- `CoherenceThresholds::validate()` rejects out-of-range values
- `ContainerSegments::validate(mode)` enforces mode-specific segment requirements
- `ContainerSegments::to_flags()` computes the bitfield from present segments
- All types are `no_std` compatible and exported from `rvf-types/src/lib.rs`

## Acceptance Test

Run the same RVF artifact on two separate machines owned by two separate teams.

**Suite:** 100 tasks (30 requiring tool use, 70 internal reasoning/memory)

**Pass criteria:**
1. Replay mode produces identical grader outputs for all 100 tasks
2. Verify mode produces at least 95/100 passing on both machines
3. Zero policy violations
4. Every externally checkable claim has evidence pointers
5. Witness chain verifies end-to-end

## References

- ADR-029: RVF Canonical Format (segment model, wire format, manifest)
- ADR-030: Cognitive Container (KERNEL_SEG, EBPF_SEG, three-tier execution)
- ADR-031: RVCOW Branching (COW branching, KernelBinding)
- ADR-033: Progressive Indexing Hardening (quality framework, coherence gates, safety budgets)
- ADR-034: QR Cognitive Seed (portable bootstrap, zero-dep crypto)
- ADR-035: Capability Report (witness bundles, scorecards, governance)
- RVF format specification (rvf-types, rvf-runtime, rvf-manifest)
- RFC 8032: Ed25519
- FIPS 180-4: SHA-256
- Dynamic minimum-cut (arXiv preprint referenced in RuVector mincut crate)

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-02-15 | ruv.io | Initial proposal |
| 1.1 | 2026-02-15 | architecture review | Resolved open questions (domain, authority, resource budgets, coherence thresholds). Added wire format section. Added cross-references to ADR-029/030/031/033. Added AuthorityLevel enum and resource budget types. Tightened ContainerSegments validation. |
| 1.2 | 2026-02-16 | implementation review | Status updated to Partially Implemented. Documented full wire-format implementation in rvf-types/src/agi_container.rs (972 lines, 24 tests). All header types, enums, constants, and validators are implemented and exported. Domain expansion TLV tags (0x0112-0x0115) integrated. |
