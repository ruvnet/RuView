# ADR 352: Bounded proposal only social autopilot

## Status

Proposed. The Phase 1 implementation exists in
`harness/social-media/src/autopilot.js`, with CLI and MCP entry points and
focused tests. This decision does not authorize account connection, execution,
review eligibility, promotion, policy mutation, network access, credential
access, deployment, or perpetual runtime.

## Context

A useful optimization loop must make repeated work restartable without turning
an untrusted screening result into authority. The principal failure mode is a
self reinforcing loop that treats a structurally valid gate receipt as proof of
independent review, then publishes or promotes its own proposal.

ADR 349 defines the frozen experiment plan, paired dataset, gate receipt, and
screening contracts. Even a passing `FlywheelEvaluationV1` reports
`gateAuthorityVerified: false`, `reviewEligibilityEstablished: false`,
`promotionAuthorized: false`, and `causalClaimAllowed: false`. The autopilot
must preserve those invariants rather than create a new authority path around
them.

Phase 1 therefore implements a finite local batch processor. A caller invokes
it, supplies the complete proposal and evaluation inputs, receives digest only
dispositions and a restart checkpoint, and explicitly invokes it again if more
cycles are required. It is not a scheduler, daemon, connector, publisher, or
self modifying learning system.

## Inputs

The loop accepts one closed input object with these fields:

1. `runId`, a bounded identifier that binds every batch and checkpoint.

2. `maximumCycles`, an integer from 1 through 100.

3. `proposals`, an array of 1 through 100 exact proposal and evaluation
   entries.

4. An optional exact `AutopilotCheckpointV1`.

Each `OptimizationProposalV1` describes exactly one change in one of four
registered classes: `CONTENT_STRUCTURE`, `EVIDENCE_PRESENTATION`,
`TIMING_HYPOTHESIS`, or `VOICE_RULE`. It binds the registered platform,
account, identity scope, frozen experiment plan digest, paired dataset digest,
2 through 16 unique source digests, creation time, expiry, expected effect,
and rollback text. The plan and dataset digests must both occur in the source
digest set. Proposal lifetime cannot exceed 90 days.

The exact platform and account tuple must resolve in
`config/identities.json`, and its registered identity scope must match the
proposal. The evaluation platform and identity scope must then match the
proposal and its frozen experiment plan. This creates exact account authority
separation for planning, but the identity registry explicitly establishes no
write authority.

Each evaluation carries the full `ExperimentPlanV1`, paired baseline and
variant arrays, one unique normalized snapshot and frozen metric semantics
binding per pair, their dataset digest, and all four gate receipts required by
ADR 349. Each individual series is bounded at 10000 observations. The sum of
all baseline and variant observations in one batch is bounded at 20000. One
autopilot batch is restricted to one exact platform, account, and identity
scope.

All input is untrusted. Unknown outer run, entry, proposal, and evaluation
wrapper fields, sensitive material, duplicate proposal digests, malformed
timestamps, expired proposals, invalid identity bindings, and batch limit
violations fail the whole invocation before a cycle starts. Malformed nested
evaluation evidence encountered during a cycle is isolated as a rejection and
consumes that cycle.

## Decision

### One proposal per cycle

One cycle evaluates exactly one proposal entry at the current cursor. The loop
advances by at most `maximumCycles` and never beyond the proposal count. An
invalid evaluation becomes `REJECTED_INVALID_EVIDENCE`, advances the cursor by
one, and cannot stall or retry itself. A binding mismatch, failed screening,
or upstream authority drift is also a rejection.

The only non rejection disposition is
`QUEUED_FOR_INDEPENDENT_VERIFICATION`. It contains proposal and evaluation
digests only. It is an evidence collection queue, not a review queue,
approval, promotion, schedule, connector command, or execution request.

### Full batch binding

Canonical SHA 256 digests bind the complete inputs without returning their raw
payloads:

1. `proposalDigest` binds every proposal field except itself.

2. `evaluationInputDigest` binds the complete canonical evaluation input,
   including the full plan, both observation arrays, their normalized snapshot
   and metric semantics bindings, and all gate receipts.

3. `identityRegistryDigest` binds the complete loaded identity registry.

4. `scopeDigest` binds the one exact platform, account, and identity scope for
   the batch.

5. `batchDigest` binds the run identifier, identity registry and scope digests,
   and the
   ordered proposal and evaluation input digest pairs.

6. `runDigest` binds the complete `AutopilotRunV1` output before the run digest
   field is added.

Array order is significant. Object keys are canonicalized before hashing. A
changed proposal, observation, receipt, order, identity registry, or run
identifier creates a different batch binding.

The result does not echo proposal text, source material, observation arrays,
or gate records. Dispositions and queues carry digest references, registered
labels, counters, authority flags, and checkpoint state only.

### Restart checkpoint

`AutopilotCheckpointV1` binds the run, batch, identity registry, exact account
scope, next cursor, the exact processed proposal digest prefix, the prior
checkpoint digest, and its own recomputed digest. A checkpoint from a different
run, batch, registry, scope, cursor prefix, or digest fails closed.

A valid completed checkpoint against an unchanged and still valid batch is a
stable no op. It processes zero cycles, returns `BATCH_COMPLETE`, and preserves
the checkpoint. Repeating that completed resume produces the same no op run
digest; that digest is intentionally different from the earlier run that
processed the final proposal. An incomplete checkpoint resumes at its exact
next cursor and emits either `CYCLE_LIMIT` or `BATCH_COMPLETE`.

Checkpoint integrity is not checkpoint authority. Every output reports
`checkpointAuthorityVerified: false`. The local digest chain detects mutation
but cannot detect replacement with an older otherwise valid checkpoint when
the attacker controls all supplied state. An authoritative runtime must retain
the expected latest checkpoint digest and run identifier outside the process,
then compare them before resume. That state store and authenticated comparison
are not implemented in Phase 1.

### Authority and side effect invariants

Every `AutopilotRunV1` reports these fixed values:

| Field | Value |
|---|---:|
| `networkAttempted` | `false` |
| `credentialStoresRead` | `false` |
| `accountConnectionsCreated` | `0` |
| `externalActionsAttempted` | `0` |
| `executionAuthorized` | `false` |
| `reviewEligibilityEstablished` | `false` |
| `promotionAuthorized` | `false` |
| `selfMutationAuthorized` | `false` |
| `checkpointAuthorityVerified` | `false` |

The upstream evaluator must also continue to report
`gateAuthorityVerified: false`. If it produces any other authority state,
causal claim, review eligibility, promotion state, or unexpected
recommendation, the autopilot emits `REJECTED_UPSTREAM_AUTHORITY_DRIFT`.

The implementation has no outbound client, connector, credential provider,
filesystem write, execution tool, policy write, or learning promotion path.
Ruflo may generate an input proposal outside this function, but Ruflo output
has the same untrusted status and cannot alter these invariants.

### Interfaces

The local CLI interface reads one JSON object from standard input:

```bash
cd harness/social-media
node bin/cli.js autopilot run < input.json
```

The MCP interface is the read only tool `social_autopilot_run`. Its tool
description and schema expose the same cycle, entry, observation, proposal,
and checkpoint bounds. Neither interface includes an execute, approve,
promote, connect, publish, send, deploy, or mutate operation.

## Outputs

`AutopilotRunV1` contains the run, batch, scope, and identity registry digests;
start and next cursors; processed and total counts; stop reason; digest only
dispositions; a digest only independent verification queue; digest only
rejections; a checkpoint; fixed side effect and authority flags; and a
canonical run digest.

The only stop reasons are `CYCLE_LIMIT` and `BATCH_COMPLETE`. The only business
outcomes are rejection or a request for independent verification. No output
establishes that the gate issuer was authentic, the underlying evidence was
competently reviewed, the result was statistically significant, or the
proposal should be activated.

## Assumptions

1. The caller can preserve the unchanged full batch across restart.

2. Canonical SHA 256 integrity is useful for mutation detection but does not
   establish source identity, reviewer competence, or execution authority.

3. The local identity registry is reviewed planning evidence only. Its digest
   detects a change within a run, not whether the change was authorized.

4. Twenty paired observations and 5 percent lift are Phase 1 screening floors,
   not significance or causal evidence.

5. A future hosted loop needs an authenticated external checkpoint head,
   bounded queue, distinct gate issuer, retention policy, and accepted live
   connector ADR before it can claim reliable restart or external effects.

## Cost and latency budgets

These are planning estimates, not measurements:

| Quantity | SYNTHETIC estimate | Evidence limit |
|---|---:|---|
| External API cost per Phase 1 run | 0 USD | No network or connector exists |
| Local compute cost per maximum batch | Less than 0.01 USD | Excludes workstation ownership and review labor |
| One 20 pair proposal latency | Less than 10 ms | Commodity laptop assumption, not benchmarked |
| Maximum 100 entry, 20000 observation batch latency | Less than 500 ms | Excludes CLI startup and constrained host contention |
| Independent verification labor per queued proposal | 5 through 30 minutes | Depends on evidence quality and reviewer scope |
| Review labor if all 100 proposals queue | About 8 through 50 hours | No automatic approval or batching credit assumed |

Production acceptance requires measured median and 95th percentile latency at
the exact package version and host size. The largest cost is independent human
verification, not local evaluation. At 100 queued proposals the labor budget
dominates compute by several orders of magnitude, so a production queue needs
rate limits and prioritization without weakening evidence gates.

## Risks and fix paths

| Risk | Severity | Implemented control | Fix path beyond Phase 1 |
|---|---|---|---|
| A passing screen is mistaken for independent authority | Critical | All gate, review, execution, and promotion authority remains false | Use an authenticated distinct gate issuer and reviewed evidence store under a separate ADR |
| An old checkpoint is replayed | High | Exact checkpoint and prior digest validation | Retain and compare the expected latest digest in an authenticated external store |
| Identity registry content is replaced before a new run | Critical | Registry digest binds each batch and write authority remains false | Require signed registry review, manifest attestation, and separate owner approval |
| Invalid evidence consumes all available cycles | Medium | One rejection consumes one cycle and the batch remains bounded | Validate producer output before enqueue and rate limit repeated invalid sources |
| Weak or biased paired data self reinforces content choices | High | Frozen plan, dataset digest, four gates, no causal claim, no promotion | Add randomized evaluation, power analysis, holdout retention, and independent analysis |
| Digest references conceal poor source quality | High | Queue means independent verification only | Require authenticated provenance and inspectable evidence before any later transition |
| Maximum input creates memory or latency pressure | Medium | 100 entry, 10000 per series, and 20000 aggregate observation caps | Add measured resource budgets and upstream backpressure before hosting |
| A future wrapper treats queue output as executable | Critical | Output has no action payload and every execution flag is false | Keep connectors in a separate least authority service with device bound approval |

The biggest remaining uncertainty is gate provenance. The implementation can
prove structure, binding, and declared outcomes, but not who issued a receipt
or whether the evidence deserved that outcome. The minimum fix is an
authenticated independent issuer plus an externally retained evidence and
checkpoint record. Until that exists, every queued item remains untrusted.

## Consequences

The loop is deterministic, bounded, restartable within an unchanged valid
batch, and safe to expose as a read only local tool. Invalid evaluation
evidence cannot crash the remaining loop, and a completed checkpoint does not
repeat work.

The loop is intentionally not autonomous in the live social media sense. A
caller must invoke each finite batch or resume. It cannot collect fresh
metrics, discover new evidence, generate credentials, schedule itself, publish
content, or learn into canonical policy. Those omissions reduce immediate
growth automation value but prevent unverified evidence from becoming a
reputational or account safety event.

## Acceptance tests

Acceptance requires all of the following:

1. `maximumCycles` below 1 or above 100 fails, proposal counts below 1 or above
   100 fail, an individual series above 10000 fails, and aggregate observations
   above 20000 fail.

2. Each cycle processes exactly one proposal. Invalid evaluation evidence
   produces one rejection, consumes one cycle, advances the cursor, and causes
   no side effect.

3. Proposal mutation, duplicate proposal digests, unknown authority fields,
   credential material, unregistered accounts, wrong identity scopes, and
   proposal to evaluation binding mismatches fail or reject as specified.

4. The batch digest changes when any full proposal, full evaluation input,
   order, run identifier, account scope, or identity registry content changes.

5. Checkpoint mutation, changed batch replay, changed run replay, and invalid
   processed prefix replay fail closed.

6. An incomplete checkpoint resumes at the exact cursor. Chunked runs preserve
   the same disposition order as a one shot run.

7. A completed checkpoint with unchanged still valid inputs is a stable no op
   with zero processed cycles. Repeating the same completed resume produces
   the same run digest and does not alter the checkpoint.

8. A passing screen can only create
   `QUEUED_FOR_INDEPENDENT_VERIFICATION`. All other paths are rejections.

9. Upstream authority drift is rejected. Gate authority, review eligibility,
   execution, promotion, causal claims, and self mutation remain false.

10. The output contains no proposal text, observations, gate records,
    credentials, connector command, or external action payload.

11. The CLI and MCP paths return the same bounded behavior and expose no live
    action command.

12. The focused and broader gates pass:

```bash
cd harness/social-media
npm run test:autopilot-proposal-only
npm run test:optimizer-proposal-only
npm run test:mcp-read-only
npm run test:network-deny
npm test
```

One execution path, credential read, network request, unbound checkpoint,
unverified review state, promotion state, policy mutation, or echoed evidence
payload makes acceptance `FAIL`.
