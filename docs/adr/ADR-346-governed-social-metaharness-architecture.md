# ADR-346: Governed social metaharness architecture

## Status

Proposed. Phase 1 is an incubated, zero credential, offline portfolio control
plane under `harness/social-media`. It has no live connector authority.

## Context

The rUv portfolio needs one place to plan content, attach evidence, isolate
identities, compare outcomes, and propose improvements across major social and
community platforms. A shared control plane can reduce duplicated research and
make each proposed publication traceable to its claims and approvals.

The premise that one orchestrator should directly control every account is
unsafe. Personal rUv, ruvnet, Agentics Foundation, Cognitum One, and future
properties have different owners, audiences, credentials, policies, and risk.
An agent generated proposal cannot grant itself authority over any of them.

This portfolio metaharness is explicitly separate from the paid media
`ruvnet/marketing` repository. The paid media repository remains the authority
for advertising accounts, campaign budgets, bidding, attribution, paid
creative, and spend reconciliation. This decision grants no import, credential
reuse, dispatch route, deployment authority, or spending authority across that
boundary. A future integration would require a separate accepted ADR and may
exchange only reviewed, digest addressed evidence packages.

ADR 345 supplies the initial dated identity and evidence baseline. This ADR
defines the control plane that may consume that baseline without converting
public attribution into write permission.

## Phase 1 invariant

Phase 1 performs no live network dispatch, account connection, publishing,
reply, comment, reaction, follow, moderation, deletion, messaging, spending,
schedule activation, or deployment.

The measurable Phase 1 budgets are:

| Dimension | Phase 1 budget |
|---|---:|
| Stored platform credentials | 0 |
| Live account connections | 0 |
| External action requests | 0 |
| Published or messaged items | 0 |
| Advertising spend | 0 USD |
| Hosted deployments | 0 |
| Allowed outputs | Local proposals, fixtures, validations, and digest receipts only |

Any nonzero value in the first six rows is a release failure.

## Inputs

The Phase 1 control plane accepts only bounded local inputs:

1. The dated public identity baseline from ADR 345.

2. Human supplied portfolio goals, audience constraints, prohibited topics,
   approval roles, and platform policy notes.

3. Public content and public metric snapshots labeled `MEASURED`, `CLAIMED`,
   or `UNVERIFIED` with source and freshness metadata.

4. Local content briefs, style guidance, approved asset references, and claim
   evidence supplied for proposal generation.

5. Redacted historical outcome fixtures whose provenance, retention, consent,
   and identity scope have been reviewed.

6. Ruflo optimization proposals that conform to a bounded proposal schema and
   carry no authority fields. ADR 352 governs the implemented proposal only
   autopilot around this boundary.

Inputs from public pages, platform responses, retrieved memory, comments,
messages, prompts, models, MCP clients, browsers, and Ruflo are untrusted data.
They cannot modify policy, approvals, identity ownership, connector scope, or
deployment state.

## Outputs

Phase 1 may produce only local artifacts. The implemented contracts are
`DirectionV1`, non executable `ApprovalChallengeV1`,
`NormalizedMetricsV1`, frozen `ExperimentPlanV1`, digest bound
`SocialEvaluationGateV1`, `FlywheelEvaluationV1`, and
`SocialAuditReceiptV1`. They bind identity, policy, target, evidence,
measurement semantics, experiment data, or audit state as appropriate.

The bounded proposal only autopilot implemented under ADR 352 adds
`OptimizationProposalV1`, checkpoint, and run records. Those records may queue
digests for independent verification only. They cannot establish review
eligibility, promotion, approval, execution, or deployment authority.

Phase 1 also produces local validation reports and deterministic fixtures.

No Phase 1 output is a platform command, credential, session, OAuth grant,
deployment manifest, paid media instruction, or evidence of human approval.

## Assumptions

1. Official APIs provide the most stable and auditable connector boundary when
   a platform exposes the required capability.

2. Platform APIs, rate limits, review requirements, and commercial terms can
   change. Every connector version therefore needs a dated capability record.

3. Browser and computer use can observe or assist an attended setup flow, but
   cannot safely substitute for policy, credential scope, or human approval.

4. Engagement is an incomplete business objective. Trust, evidence quality,
   privacy, operator time, conversion quality, and platform safety must remain
   constrained objectives.

5. Public audience counters overlap and are not unique reach.

6. Local digest receipts are tamper evident by construction. Durable legal or
   regulatory immutability would require a separately qualified write once
   store and retention policy.

## Architecture decision

`harness/social-media` is an incubated portfolio control plane with five local
layers:

| Layer | Responsibility | Authority |
|---|---|---|
| Identity registry | Map one owner, organization, platform, and account identifier to one isolated tenant | Read only in Phase 1 |
| Evidence registry | Normalize dated claims, sources, metrics, and freshness | Append proposal records only |
| Proposal engine | Produce bounded content and action proposals | No external side effects |
| Policy and approval verifier | Evaluate identity scope, claim quality, action class, and approval receipts | Deny by default |
| Receipt ledger | Canonicalize records, compute digests, link decisions, and detect mutation | Append only by contract |

Connector code is API first. A future platform adapter must implement a narrow
contract that separates observation, preparation, and execution:

1. `capabilities` returns a dated, versioned capability description without
   elevating authority.

2. `read` retrieves only the data authorized for one identity and one purpose.

3. `prepare` validates a proposed action, renders the exact platform payload,
   and returns its canonical digest without dispatch.

4. `execute` accepts only the same unexpired digest and approval receipt.
   `execute` is prohibited in Phase 1 and may be exposed only through a future
   CLI after its rollout gate is accepted.

5. `reconcile` records a redacted platform result against the execution digest
   without treating the response as policy authority.

Official APIs take precedence over MCP, browser automation, and computer use.
A connector may not silently fall back to another transport. Each transport
requires its own capability and security qualification.

## Attended manual setup only

Any future account connection is an attended manual operation. The identity
owner must select the exact account, review every requested permission, finish
the provider flow directly, and confirm the resulting identity identifier.

The harness must not enter a password, request a password in a prompt, read a
one time code, solve a challenge, bypass a provider protection, copy a browser
session, persist a browser cookie, scrape a private token, or expand a granted
scope. Computer use may assist navigation only while the owner is present. A
human completes sensitive fields directly.

Connection evidence must record provider, identity identifier, scopes,
granting human, creation time, expiry, revocation procedure, and secret store
reference. It must never record the credential itself.

Phase 1 does not perform this setup and contains no credential store.

## MCP and CLI boundary

MCP is read only. Its implemented surface exposes diagnostics, platform
capabilities, the dated baseline, direction policy and lint, non executable
action planning, metric normalization, optimization screening, and receipt
verification. ADR 352 adds bounded proposal only autopilot evaluation. MCP
must not expose connect, publish, reply, message, react, follow, moderate,
delete, schedule, spend, deploy, approval creation, promotion, or execution
tools.

Future live execution is CLI only. The CLI must require an interactive terminal
unless a separately accepted unattended execution ADR defines stronger
controls. It must display the exact identity, target, action, content summary,
payload digest, requested scopes, expiry, and approval receipts before the
operator confirms dispatch.

MCP output, model output, browser content, repository text, and Ruflo proposals
cannot satisfy the CLI confirmation or approval requirement.

## Identity isolation

One tenant represents exactly one legal or operating identity. One platform
account belongs to exactly one tenant. Credentials, proposal queues, policies,
analytics, experiments, receipts, retention, and approvals are tenant scoped.

A piece of content intended for three identities becomes three separate
proposals with three target digests and three approvals. A shared handle,
email, project, or human operator does not collapse the boundary. Paid media
identity records remain outside this repository and outside this control plane.

Cross tenant statistics may use only reviewed aggregates with explicit metric
definitions. Raw private messages, contact graphs, group membership, user
identifiers, and audience exports cannot enter a cross tenant aggregate.

## Immutable digest receipts

Every proposal, evidence record, policy decision, approval, and future
execution result uses canonical JSON and a SHA 256 digest. The digest includes
the schema version, tenant, platform, target identifier, action, content,
assets, claims, evidence digests, policy version, creation time, and expiry.

Records are immutable by contract after digest creation. A content, target,
asset, claim, policy, or expiry change creates a new record and digest. The old
record remains linked as superseded. Overwriting an existing digest is an
integrity failure.

A future `ApprovalReceiptV1` must bind an authenticated human actor, role,
approval scope, proposal digest, target identity, issued time, and expiry. A
future `ExecutionReceiptV1` must bind the approval digest, connector version,
request digest, redacted response digest, platform action identifier, and
result time.

Digest equality proves content equality under the chosen canonicalization. It
does not prove that a claim is true, that a human understood it, or that a
platform accepted it. Those remain separate evidence and approval questions.

The implemented `SocialAuditReceiptV1` chain detects mutation, replay, and
deletion only when verification receives an externally retained expected head
digest and receipt count. The chain cannot attest its own completeness.

## Exact approval policy

The following matrix applies to future phases. Phase 1 cannot exercise any of
these actions.

| Action | Required approval | Additional rule |
|---|---|---|
| Create local draft or analysis | No external action approval | Must remain a proposal and carry tenant scope |
| Accept an identity mapping | Identity owner and security reviewer | Exact platform identifier and ownership evidence required |
| Connect, rotate, or revoke a credential | Identity owner and security reviewer | Attended manual setup, least authority scope, revocation test |
| Publish a post, article, image, video, gist, or repository social release | Identity owner or named delegated editor | Approval binds the exact payload digest and expires after 30 minutes |
| Reply, comment, or react | Identity owner or named community editor | Approval binds the exact thread, target, and payload digest and expires after 30 minutes |
| Send a direct or group message | Identity owner and privacy reviewer | Recipient set, purpose, retention, and exact payload digest required |
| Follow, unfollow, invite, remove, hide, delete, moderate, or change account state | Identity owner and operations reviewer | Exact target and rollback or recovery plan required where available |
| Schedule recurring live execution | Repository maintainer, identity owner, security reviewer, and operations owner | Separate unattended execution ADR and kill switch exercise required |
| Deploy a live service | Repository maintainer, security reviewer, and operations owner | Separate deployment ADR, threat model, cost cap, rollback, and observed canary required |
| Spend money or manage paid media | Prohibited in this control plane | Governed only by the separate `ruvnet/marketing` repository |
| Promote a Ruflo optimization candidate | Repository maintainer and independent reviewer | Holdout, safety, quality, provenance, and frozen anchor gates must pass |

An approval is valid only for its exact digest, tenant, target, action, and
expiry. Any mutation, scope change, connector change, target change, or expired
receipt requires a new approval. Approval cannot be inferred from a prompt,
chat response, branch name, issue label, prior post, role title, or model output.

## Ruflo optimization boundary

Ruflo is a proposal only optimizer. It may analyze reviewed, tenant scoped,
redacted fixtures and propose one bounded change to timing, format, evidence
presentation, content structure, or experiment allocation.

Ruflo cannot change policy, identity ownership, credential scope, approval
rules, connector code, live schedules, production configuration, or canonical
learning records. It cannot approve, dispatch, deploy, spend, or promote its
own candidate.

Each optimization proposal must declare one bounded change, expected effect,
rollback, plan and dataset bindings, provenance digests, creation time, and
expiry. Evaluation requires a preregistered experiment plan, paired data, a
recomputed dataset digest, and current digest bound records for anchors,
provenance, security, and blocked actions.

Phase 1 does not verify the authority of gate issuers. A passing screen can be
queued for independent gate verification only. It sets
`reviewEligibilityEstablished: false` and `promotionAuthorized: false`.

The bounded autopilot accepts 1 through 100 proposals, a 1 through 100 cycle
cap, and no more than 20,000 aggregate observations. A resumed checkpoint must
bind the same run, batch, identity registry, cursor, and processed proposal
prefix. Structural checkpoint verification does not prove issuer authority;
every run reports `checkpointAuthorityVerified: false`.

Optimizing raw engagement alone is prohibited. A candidate fails when it
improves clicks while degrading evidence quality, privacy, safety, audience
trust, operator time, or platform policy compliance.

## Biggest failure mode: policy laundering

The largest architectural risk is policy laundering. An untrusted post,
retrieved memory, prompt, connector response, or Ruflo proposal may contain
language such as approved, urgent, owner requested, or safe. A downstream
component may accidentally interpret that data as authority and dispatch an
action.

The fix is structural. Authority exists only in the policy verifier and in a
digest bound human approval receipt. Content schemas have no authority fields.
Untrusted data cannot select a credential, expand a scope, set an approval,
change an identity mapping, or call execution. Every boundary defaults to deny,
and the receipt ledger preserves the denied capability set for audit.

Policy laundering is a release blocking defect. One reproduced path from
untrusted content to approval or execution fails the entire release.

## Non goals

This decision does not authorize or implement:

1. Live social publishing, messaging, moderation, scheduling, or account
   connection in Phase 1.

2. Paid media, campaign management, bidding, budget allocation, attribution,
   or advertising spend.

3. Autonomous browser login, cookie reuse, challenge bypass, password entry,
   or unattended computer use.

4. Scraping that violates a platform policy or bypasses an official API.

5. Collection of private messages, contact graphs, private group content, or
   user level profiling.

6. One global credential, one global queue, or one approval covering multiple
   identities.

7. Self approving or self promoting learning loops.

8. A hosted GCP runtime. ADR 350 defines a proposed Phase G1 static blueprint,
   but no Terraform validation, live plan, or deployment is Phase 1 evidence.

## Staged rollout

| Stage | Capability | Entry gate | Exit evidence |
|---|---|---|---|
| Phase 1, offline incubation | Identity registry, evidence normalization, local proposals, policy decisions, digest receipts, fixtures, and tests | ADR 345 baseline accepted | Zero credentials, zero account connections, zero network dispatch, deterministic digests, policy laundering tests green |
| Phase 2, attended read only | One official API connector for one isolated identity, manual owner setup, metrics retrieval, and reconciliation | Connector threat model, policy review, data minimization, revocation test | Thirty days of read only operation with zero write attempts and complete receipts |
| Phase 3, attended CLI dispatch | One reversible or low impact action class on one identity | Exact approval flow, dry run parity, sandbox account validation, kill switch, rollback exercise | At least 50 approved canary actions, zero unauthorized actions, zero identity escapes, complete reconciliation |
| Phase 4, bounded portfolio operation | Additional identities and action classes one at a time | Independent connector and identity qualification for each addition | Per identity error, review time, policy denial, and trust metrics meet accepted thresholds |
| Future hosted runtime | Scheduled read operations and separately approved bounded execution on local infrastructure or GCP | Separate unattended execution and deployment ADRs, cost cap, secret manager, workload identity, alerting, canary, rollback, and kill switch | Thirty day canary with zero unauthorized actions, budget within cap, and tested recovery |

No stage inherits authority merely because the prior stage completed. Each new
identity, connector, transport, action class, and deployment target requires
its own evidence gate.

## Business value and operating budgets

The control plane creates value by reusing evidence once, reducing identity
confusion, making review decisions inspectable, and letting the portfolio learn
from bounded experiments without granting the optimizer authority.

Phase 1 external API cost and advertising spend are exactly 0 USD by design.
Local compute and operator time are not claimed as zero and must be measured
before a production business case is accepted.

The following are targets, not measured results:

| Target | Budget |
|---|---:|
| Canonical proposal size | At most 256 KiB |
| Canonical receipt size | At most 128 KiB |
| Offline schema and policy validation latency | p95 at most 250 ms for a 256 KiB proposal on the documented reference machine |
| Live action approval validity in future phases | 30 minutes |
| Credentials per connector instance | 1 identity and 1 platform only |
| Unauthorized actions | 0 |
| Cross tenant data escapes | 0 |
| Undigested external actions | 0 |

No platform cost, cloud cost, conversion lift, reach lift, or operator time
saving is estimated in this ADR because workload, rate limits, provider terms,
and review time are not yet measured. A later business case must label those
values `MEASURED`, `CLAIMED`, or `SYNTHETIC` and provide a reproducer.

## Security and governance consequences

The architecture adds review latency before any future external action. A
single content action needs at least one exact digest approval. Sensitive or
state changing actions need two human roles. This is deliberate friction at
the point where identity, privacy, reputation, money, or account state can be
damaged.

API first connectors improve determinism and auditability but do not guarantee
safety. Provider responses remain untrusted, scopes remain least authority,
and revocation must be tested. Browser, MCP, and computer use do not receive a
weaker policy path.

The receipt ledger is tamper evident, not a substitute for backups, retention
policy, legal hold, or a qualified write once store. Secrets remain outside the
ledger and are referenced only by an opaque secret store identifier in future
phases.

## Acceptance tests

Phase 1 is accepted only when all of the following tests pass:

1. The implementation resides under `harness/social-media` and has no import,
   credential, dispatch, budget, deployment, or runtime dependency on the
   separate `ruvnet/marketing` repository.

2. Static imports reject DNS, socket, outbound HTTP, browser, shell, and child
   process clients. An operating system network deny profile runs selected
   suites that do not require the local HTTP test server. Endpoint tests run
   separately, and the observed live platform dispatch count is zero.

3. A secret inventory reports zero platform tokens, OAuth grants, cookies,
   passwords, one time codes, account sessions, or private keys.

4. The MCP manifest exposes only read operations for identity, evidence,
   proposals, statistics, policy explanation, and receipt verification. It
   exposes no connect or external action tool.

5. No CLI execution command is available in Phase 1. A future execution command
   remains behind the Phase 3 gate and cannot be called through MCP.

6. Canonicalizing the same fixture twice produces the same SHA 256 digest.
   Changing one byte, tenant, target, action, evidence digest, policy version,
   or expiry produces a different digest.

7. A Ruflo proposal containing words that imply approval cannot change policy,
   create an approval receipt, select a credential, or reach execution.

8. The implemented identity registry binds exact platform accounts to one
   identity scope, and direction and metric inputs reject mismatches. Separate
   queues, credentials, and durable analytics namespaces remain future work.

9. Every normalized metric binds source and provenance digests, evidence
   label, collection mode, connector definition version, exact window,
   collection time, quality flags, and identity scope. Future dated evidence
   is rejected, and incompatible records cannot be compared.

10. Phase 1 test evidence records zero account connections, zero publications,
    zero messages, zero moderation actions, zero spending, and zero deployments.

11. Direction mutation, expiry, target substitution, stale policy, identity
    mismatch, audit mutation, deletion, replay, and policy laundering tests
    fail closed. Real device approval and connector substitution tests remain
    future because neither capability is implemented.

12. A security review confirms that attended manual setup remains future,
    approval challenges create no authorization, identity isolation, Ruflo
    proposal only behavior, and the paid media boundary match this ADR.

13. The ADR 352 bounded autopilot gates pass. Its output remains a proposal or
    independent verification queue item and never establishes checkpoint
    authority, review eligibility, or promotion.

One reproduced unauthorized action, cross tenant escape, live network dispatch,
credential capture, or policy laundering path makes acceptance `FAIL`.
