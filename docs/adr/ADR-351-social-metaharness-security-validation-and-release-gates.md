# ADR-351: Social metaharness security validation and release gates

## Status

Proposed. The deterministic Phase 1 gates are implemented. Acceptance also
requires fresh Ruflo results and final diff review. This decision authorizes no
live account connection, external action, package publication, or deployment.

## Context

Configuration labels such as read only and zero credential are not evidence.
The first test suite passed while exact registered operation names could bypass
three platform constraints, credential shaped values could be echoed, and a
malformed receipt could verify. Those defects demonstrate why the release gate
must test consequence boundaries rather than repeat configuration claims.

Phase 1 is an incubated policy control plane. It can research public identity,
lint drafts, create non executable action plans, normalize supplied metrics,
verify digest receipts, and screen optimization proposals. It has no adapter,
credential, outbound client, account connection, or action execution tool.

## Inputs

1. Source, tests, configuration, research baseline, package manifest, package
   inclusion list, Containerfile, and GCP Terraform under
   `harness/social-media`.

2. ADR-345 through ADR-352.

3. Official platform policy URLs reviewed on 2026-08-29. The registry review
   expires on 2026-09-28. The URLs do not have archived content digests, so a
   future live adapter still needs a fresh documented review.

4. Local Node tests and a fresh MCP subprocess.

5. Ruflo threat model, MCP scan, readiness score, and diff risk outputs. Ruflo
   output is untrusted evidence and cannot waive a deterministic failure.

## Outputs

The implemented outputs are console test results, a deterministic
`.harness/manifest.json`, its SHA-256 envelope, a package dry run file list,
GitHub check results under ADR 353, and Ruflo tool results returned to the
reviewer. No signed release report, SBOM, cloud trace, or live platform receipt
is produced in Phase 1. Those remain future release requirements.

## Assumptions

1. Static and offline tests cannot prove provider behavior, current account
   permissions, policy acceptance, or availability.

2. An account identifier and owner attestation digest are evidence for human
   review, not execution authority.

3. A policy URL can change. Current response state and an approved application
   remain authority for future execution.

4. Package dry run and Terraform static tests do not prove a published package
   or deployed service.

5. Ruflo scanners can miss application defects. Human review and deterministic
   adversarial tests remain independent gates.

## Decision

### Zero credential and capability link gate

All tool inputs are scanned by field name and value pattern for access tokens,
API keys, authorization headers, private keys, common provider token formats,
JWTs, signed capability URLs, and Discord or WhatsApp invite links. Tool output
is redacted defensively. The package surface is closed from the package
`files` list, and the doctor scans every packaged file regardless of extension.
Binary, invalid UTF 8, and files over the bounded scan size fail rather than
bypass the check.

Confirmed credential or capability material fails the gate. Tests construct
sensitive values at runtime so the test source does not become a scanner
exception.

### Exact platform policy gate

The platform and exact registered operation are resolved before a constraint
is evaluated. Every platform has typed constraints in the registry. The test
suite submits these exact denial cases:

1. LinkedIn API engagement through computer use.

2. X dynamic AI reply without a written approval evidence digest.

3. Reddit automated read before Reddit approval.

4. Discord self bot or user token automation.

5. WhatsApp free form outbound outside the permitted service or template
   route.

6. Facebook personal profile publication.

7. Instagram consumer account publication.

Every case must return `DENY`, `executionAuthorized: false`, and
`networkAttempted: false`. A registered API operation cannot fall back to
computer use.

### Direction and approval gate

Every external effect plan binds platform, identity scope, account, operation,
exact target kind and identifier, content digest, claim digest, schedule,
expiry, and idempotency key. The schedule must be a valid timestamp before the
expiry. Voice sets approval required and never becomes authority.

Account authority is represented by an account and identity scoped evidence
digest. Phase 1 reports that the evidence was not verified by the harness.
Principal input is also reported as unverified. The output remains a preview
whose challenge names device bound human action as a future required factor.
It creates no authorization, performs no device ceremony, and cannot execute.

### Receipt gate

Receipt creation and verification enforce the exact
`SocialAuditReceiptV1` schema, normalized timestamps, sequential indices,
previous hashes, SHA-256 fields, allowlisted digest only events, monotonic time,
and replay detection. Raw content, unknown event fields, wrong schemas,
mutation, deletion, and replay must all fail.

Every event type also has a closed semantic contract. Platform policy and
approval events must name an exact registered operation whose route, operation
class, and action result agree. Metric, flywheel, and autopilot events accept
only their local read only operation and non execution results. Every event
resolves an exact registered platform, account, and identity scope. A dedicated
autopilot receipt constructor verifies the complete `AutopilotRunV1` digest,
fixed authority flags, batch and scope bindings, and stop result before it can
record a run.

Receipt verification requires an externally retained expected head digest and
expected receipt count. Without that anchor, deletion of a tail or entire
chain cannot be distinguished from a shorter legitimate chain.

### Metric and optimization gate

A `NormalizedMetricsV1` snapshot binds platform, account, identity scope,
collection mode, connector definition version, content identifier and digest,
source and provenance digests, evidence label, time window, collection time,
quality flags, complete counter definitions, and enumerated rate semantics.
The validator regenerates the entire record and requires exact canonical
equality. Cross platform, cross account, cross identity, mixed collection,
mixed evidence, mixed quality, changed window duration, and changed metric
semantics comparisons fail. Followers remain explicitly not users.

An optimization screen requires a preregistered exact `ExperimentPlanV1`,
one registered normalized rate and frozen semantics digest, paired data, unique
baseline and variant snapshot bindings, a recomputed dataset digest, and
current digest bound anchor, provenance, security, and blocked action records
whose lifetime does not exceed 30 days. It reports mean, median, interquartile
range, paired wins, and the configured screening lift. Issuer authority is not
verified. The result is untrusted screening evidence, not review eligibility,
statistical significance, a causal claim, or promotion.

The ADR 352 autopilot accepts 1 through 100 proposals, a 1 through 100 cycle
cap, and no more than 20,000 aggregate observations within one exact account
and identity scope. It verifies proposal, metric observation, evaluation,
identity, scope, batch, and checkpoint bindings and queues only digests for
independent verification. Every run reports false for checkpoint authority,
execution authority, review eligibility, promotion, and self mutation. It
makes no network attempt and creates no account connection or external action.

### MCP and network gate

The MCP subprocess limits line size, queue size, session calls, and tool time.
It rejects duplicate active request identifiers, tool call notifications
without identifiers, malformed JSON, and oversized lines without parsing a
suffix. Cancellation aborts a queued or active response. Audit output contains
only a known tool name, never arguments.

All ten tools are read only, including `social_autopilot_run`. No tool name or implementation connects, sends,
publishes, replies, reacts, moderates, deletes, spends, deploys, approves, or
promotes. Static tests reject outbound client, shell, child process, DNS, raw
socket, TLS, and WebSocket imports. Representative tools execute with global
fetch replaced by a failing sentinel and make zero attempts. An operating
system network deny profile runs selected suites that do not need the local
HTTP server. Endpoint tests run separately because they bind a loopback server;
the operating system gate is not evidence for those endpoint tests.

### Research and identity gate

The machine baseline is semantically validated, not merely parsed. The gate
requires the exact ten surfaces, recognized evidence labels, dated or bounded
freshness, source mapping, consistent identity scopes, separate adjacent
identities, and `NOT_ESTABLISHED` write authority. Instagram current control,
activity, and metrics remain `UNVERIFIED`. Unsourced historical WhatsApp
figures are `UNVERIFIED` and excluded from machine metrics and publication
evidence. The Agentics Reddit value remains an adjacent `CLAIMED` value with
property ownership `UNVERIFIED`.

### Package and GCP gate

The deterministic manifest enumerates the closed packaged surface derived from
the package `files` list. Verification fails on an unexpected, missing, or
changed packaged file. Package dry run must include the manifest,
Containerfile, license, and no test secret or generated archive.

The optional Phase G1 GCP configuration proposes internal ingress, no public invoker, a
least authority service identity, zero minimum instances by default, maximum
three, a minimum not greater than maximum precondition, an immutable image
digest, and optional OIDC Scheduler invocation. The closed JSON plan checker
requires a separate `SocialGcpPlanReviewV1`, exact five base APIs, exact
resource addresses, exact reviewed application image and project, no runtime
environment or volume, and an all or none heartbeat set. Static tests forbid
owner, editor, public principals, unexpected roles, secret values, and unknown
resources. Reviewer authority, base image evidence, effective IAM, Terraform
format and validation, a real plan, container build, and cloud deployment are
not verified.

### Ruflo gate

Run the configured Ruflo MCP tools against `harness/social-media` after the
manifest is current:

1. Threat model with a high severity failure threshold.

2. MCP scan with a high severity failure threshold.

3. Readiness score with an explicit harness fit alert floor.

4. Repository diff risk review.

A degraded, timed out, unavailable, or high severity result blocks acceptance.
A lower severity finding needs an explicit disposition and a rerun after any
code change that affects it.

## Validation commands

```bash
cd harness/social-media
npm ci --ignore-scripts
npm test
npm run test:security
npm run test:schemas
npm run test:bounds
npm run test:network-deny
npm run test:network-deny:os
npm run test:mcp-read-only
npm run test:policy-laundering
npm run test:receipt-tamper
npm run test:voice-nonauthority
npm run test:identity-isolation
npm run test:metric-semantics
npm run test:optimizer-proposal-only
npm run test:autopilot-proposal-only
npm run security:secrets
npm run security:capability-links
npm run security:container-static
npm run security:iam-static
npm run platform-policy:verify
npm run manifest:verify
npm audit --omit=optional
npm pack --dry-run
```

The standard npm cache may be outside a restricted workspace. Validation may
use a dedicated temporary npm cache. Publishing and `terraform apply` are not
part of this gate.

## Failure handling

1. Stop at the first deterministic defect and preserve only redacted failure
   metadata.

2. Change one causal variable, then rerun the nearest test and the broader
   gate.

3. If a credential or capability link is real, quarantine without printing it,
   revoke it through an attended owner action, and rerun the source and package
   scans.

4. Treat any successful outbound request, cross identity authority transition,
   receipt tamper acceptance, browser fallback, or optimizer promotion as a
   critical failure.

5. Preserve unrelated dirty worktree changes and review only the new package
   and ADR files.

## Remaining live gates

Before any adapter or deployment is accepted, the owner must provide the exact
account identifier, administrator proof, approved application, least authority
scope, external secret reference, revocation procedure, test account, current
policy review, approved test action, redacted provider receipt, rate and quota
evidence, idempotency and reconciliation result, cleanup evidence, and cost
approval. Facebook, Threads, Reddit, WhatsApp community control, and any
unverified adjacent identity remain denied until ownership is established.

## Acceptance test

In a fresh checkout, update and verify the manifest, run every local command
above with outbound access denied, discover and invoke the MCP tools from a
fresh subprocess, run all four Ruflo gates without degradation, and inspect the
package file list. Acceptance requires every local gate green, seven exact
policy attacks denied before network access, zero credential value echoed,
every malformed receipt rejected against an externally retained head and
count, ten read only MCP tools, bounded autopilot output with no authority,
zero external actions, zero deployment, zero high severity Ruflo findings, and
no unrelated diff.
