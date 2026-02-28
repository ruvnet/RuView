# ADR-035: Capability Report — Witness Bundles, Scorecards, and Governance

**Status**: Implemented
**Date**: 2026-02-15
**Depends on**: ADR-034 (QR Cognitive Seed), SHA-256, HMAC-SHA256

## Context

Claims without evidence are noise. This ADR defines the proof infrastructure:
a signed, self-contained witness bundle per task execution, aggregated into
capability scorecards, and governed by enforceable policy modes.

The acceptance test: run 100 real repo issues with a fixed policy.
"Prove capability" means 60+ solved with passing tests, zero unsafe actions,
and every solved task has a replayable witness bundle.

## 1. Witness Bundle

### 1.1 Wire Format

A witness bundle is a binary blob: 64-byte header + TLV sections + optional
32-byte HMAC-SHA256 signature.

```
+-------------------+-------------------+-------------------+
| WitnessHeader     | TLV Sections      | Signature (opt)   |
| 64 bytes          | variable          | 32 bytes          |
+-------------------+-------------------+-------------------+
```

### 1.2 Header Layout (64 bytes, `repr(C)`)

| Offset | Type      | Field                    |
|--------|-----------|--------------------------|
| 0x00   | u32       | magic (0x52575657 "RVWW")|
| 0x04   | u16       | version (1)              |
| 0x06   | u16       | flags                    |
| 0x08   | [u8; 16]  | task_id (UUID)           |
| 0x18   | [u8; 8]   | policy_hash              |
| 0x20   | u64       | created_ns               |
| 0x28   | u8        | outcome                  |
| 0x29   | u8        | governance_mode           |
| 0x2A   | u16       | tool_call_count          |
| 0x2C   | u32       | total_cost_microdollars  |
| 0x30   | u32       | total_latency_ms         |
| 0x34   | u32       | total_tokens             |
| 0x38   | u16       | retry_count              |
| 0x3A   | u16       | section_count            |
| 0x3C   | u32       | total_bundle_size        |

### 1.3 TLV Sections

Each section: `tag(u16) + length(u32) + value(length bytes)`.

| Tag    | Name          | Content                                      |
|--------|---------------|----------------------------------------------|
| 0x0001 | SPEC          | Task prompt / issue text (UTF-8)             |
| 0x0002 | PLAN          | Plan graph (text or structured)              |
| 0x0003 | TRACE         | Array of ToolCallEntry records               |
| 0x0004 | DIFF          | Unified diff output                          |
| 0x0005 | TEST_LOG      | Test runner output                           |
| 0x0006 | POSTMORTEM    | Failure analysis (if outcome != Solved)      |

Unknown tags are ignored (forward-compatible).

### 1.4 ToolCallEntry (variable length)

| Offset | Type      | Field              |
|--------|-----------|--------------------|
| 0x00   | u16       | action_len         |
| 0x02   | u8        | policy_check       |
| 0x03   | u8        | _pad               |
| 0x04   | [u8; 8]   | args_hash          |
| 0x0C   | [u8; 8]   | result_hash        |
| 0x14   | u32       | latency_ms         |
| 0x18   | u32       | cost_microdollars  |
| 0x1C   | u32       | tokens             |
| 0x20   | [u8; N]   | action (UTF-8)     |

### 1.5 Signature

HMAC-SHA256 over the unsigned payload (header + sections, before signature).
Same primitive used by ADR-034 QR seeds. Zero external dependencies.

### 1.6 Evidence Completeness

A witness bundle is "evidence complete" when it contains all three:
SPEC + DIFF + TEST_LOG. Incomplete bundles are valid but reduce the
evidence coverage score.

## 2. Task Outcomes

| Value | Name    | Meaning                                       |
|-------|---------|-----------------------------------------------|
| 0     | Solved  | Tests pass, diff merged or mergeable          |
| 1     | Failed  | Tests fail or diff rejected                   |
| 2     | Skipped | Precondition not met                          |
| 3     | Error   | Infrastructure or tool failure                |

## 3. Governance Modes

Three enforcement levels, each with a deterministic policy hash:

### 3.1 Restricted (mode=0)

- **Read-only** plus suggestions
- Allowed tools: Read, Glob, Grep, WebFetch, WebSearch
- Denied tools: Bash, Write, Edit
- Max cost: $0.01
- Max tool calls: 50
- Use case: security audit, code review

### 3.2 Approved (mode=1)

- **Writes allowed** with human confirmation gates
- All tool calls return PolicyCheck::Confirmed
- Max cost: $0.10
- Max tool calls: 200
- Use case: production deployments, sensitive repos

### 3.3 Autonomous (mode=2)

- **Bounded authority** with automatic rollback on violation
- All tool calls return PolicyCheck::Allowed
- Max cost: $1.00
- Max tool calls: 500
- Use case: CI/CD pipelines, nightly runs

### 3.4 Policy Hash

SHA-256 of the serialized policy (mode + tool lists + budgets), truncated
to 8 bytes. Stored in the witness header. Any policy change produces a
different hash, preventing silent drift.

### 3.5 Policy Enforcement

Tool calls are checked at record time:

1. Deny list checked first (always blocks)
2. Mode-specific check:
   - Restricted: must be in allow list
   - Approved: all return Confirmed
   - Autonomous: all return Allowed
3. Cost budget checked after each call
4. Tool call count budget checked after each call
5. All violations recorded in the witness builder

## 4. Scorecard

Aggregate metrics across witness bundles.

| Metric                    | Type  | Description                           |
|---------------------------|-------|---------------------------------------|
| total_tasks               | u32   | Total tasks attempted                 |
| solved                    | u32   | Tasks with passing tests              |
| failed                    | u32   | Tasks with failing tests              |
| skipped                   | u32   | Tasks skipped                         |
| errors                    | u32   | Infrastructure errors                 |
| policy_violations         | u32   | Total policy violations               |
| rollback_count            | u32   | Total rollbacks performed             |
| total_cost_microdollars   | u64   | Total cost                            |
| median_latency_ms         | u32   | Median wall-clock latency             |
| p95_latency_ms            | u32   | 95th percentile latency               |
| total_tokens              | u64   | Total tokens consumed                 |
| total_retries             | u32   | Total retries across all tasks        |
| evidence_coverage         | f32   | Fraction of solved with full evidence |
| cost_per_solve            | u32   | Avg cost per solved task              |
| solve_rate                | f32   | solved / total_tasks                  |

### 4.1 Acceptance Criteria

| Metric              | Threshold | Rationale                        |
|----------------------|-----------|----------------------------------|
| solve_rate           | >= 0.60   | 60/100 solved                    |
| policy_violations    | == 0      | Zero unsafe actions              |
| evidence_coverage    | == 1.00   | Every solve has witness bundle   |
| rollback_correctness | == 1.00   | All rollbacks restore clean state|

## 5. Deterministic Replay

A witness bundle contains everything needed to verify a task execution:

1. **Spec**: What was asked
2. **Plan**: What was decided
3. **Trace**: What tools were called (with hashed args/results)
4. **Diff**: What changed
5. **Test log**: What was verified
6. **Signature**: Tamper proof

Replay flow:
1. Parse bundle, verify signature
2. Display spec and plan
3. Walk trace entries, showing each tool call
4. Display diff
5. Display test log
6. Verify outcome matches test log

## 6. Cost-to-Outcome Curve

Track over time (nightly runs):

| Week | Tasks | Solved | Cost/Solve | Tokens/Solve | Retries | Regressions |
|------|-------|--------|------------|--------------|---------|-------------|
| 1    | 100   | 60     | $0.015     | 8,000        | 12      | 0           |
| 2    | 100   | 64     | $0.013     | 7,500        | 10      | 1           |
| ...  | ...   | ...    | ...        | ...          | ...     | ...         |

A stable downward slope on cost/solve with flat or rising success rate
is the compounding story.

## Implementation

| File                                          | Purpose                 | Tests |
|-----------------------------------------------|-------------------------|-------|
| `crates/rvf/rvf-types/src/witness.rs`         | Wire-format types       | 10    |
| `crates/rvf/rvf-runtime/src/witness.rs`       | Builder, parser, score  | 14    |
| `crates/rvf/rvf-runtime/tests/witness_e2e.rs` | E2E integration         | 11    |

All tests use real HMAC-SHA256 signatures. Zero external dependencies.

## References

- ADR-034: QR Cognitive Seed (SHA-256, HMAC-SHA256 primitives)
- FIPS 180-4: Secure Hash Standard (SHA-256)
- RFC 2104: HMAC (keyed hashing)
- RFC 4231: HMAC-SHA256 test vectors
