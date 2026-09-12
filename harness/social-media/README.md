# RuV social metaharness

The Phase 1 metaharness is a working zero credential control plane. It turns
public research, platform rules, voice direction, draft evidence, metrics, and
Ruflo screening into reviewable action plans. It does not connect or operate a
social account.

## What exists

| Capability | Phase 1 state | Authority |
|---|---|---|
| Public RuVnet identity baseline | Implemented | Dated public evidence only |
| Platform capability registry | Implemented | API, attended manual, or deny |
| Voice and direction lint | Implemented | Voice never approves |
| Action plan and approval challenge | Implemented | Non executable |
| Metric normalization | Implemented | Same platform, account, identity, collection mode, and semantics only |
| Optimization screening | Implemented | Untrusted screening evidence only |
| Bounded autopilot | Implemented under ADR 352 | Proposal and independent verification queue only |
| Digest receipt chain | Implemented | Verification requires an externally retained expected head and count |
| MCP server | Implemented | Read only tools only |
| Cloud Run control plane | Optional blueprint | Internal, read only, no secrets |
| OAuth and platform adapters | Not implemented | Blocked on owner and app evidence |
| Publishing, messages, moderation, spend | Not implemented | Blocked |

The paid media project at `ruvnet/marketing` is a separate system. Its campaign
claims and credentials are not imported into this organic social control plane.

## Control flow

```text
public evidence
  -> channel voice and claim lint
  -> DirectionV1
  -> deterministic platform policy
  -> non executable action preview
  -> normalized metrics
  -> frozen experiment plan and digest bound gate evidence
  -> Ruflo proposal
  -> bounded proposal only autopilot screening under ADR 352
  -> independent verification queue
```

Device bound approval, isolated adapters, dispatch, and platform receipts are
future stages. They are not part of this control flow in Phase 1.

Computer use is limited to attended setup or a separately performed manual
action when the platform registry permits it. It is never an automatic fallback
for an unavailable or prohibited API operation.

## Run locally

Node 20 or newer is required. The package has no runtime dependencies.

```bash
cd harness/social-media
npm test
node bin/cli.js doctor --strict
node bin/cli.js platforms
node bin/cli.js research baseline
node bin/cli.js mcp start
```

Structured commands accept one bounded JSON object on standard input:

```bash
node bin/cli.js direction check < draft.json
node bin/cli.js action plan < action.json
node bin/cli.js metrics normalize < metrics.json
node bin/cli.js flywheel evaluate < experiment.json
node bin/cli.js autopilot run < autopilot.json
node bin/cli.js audit verify < receipts.json
```

No command accepts a token, secret, password, cookie, API key, or private key.
The MCP surface has no connect, send, publish, reply, react, moderate, delete,
spend, deploy, approve, or promote tool.

## Identity and content direction

Personal rUv, ruvnet, Agentics, and Cognitum are distinct identity scopes. A
content plan must name one exact scope and account. Sharing the same founder or
project does not grant cross account publishing authority.

Quantitative content must include an evidence record labelled `MEASURED`,
`CLAIMED`, or `SYNTHETIC`, an HTTPS source, and a measurement timestamp.
`MEASURED` content should include a reproducer. Flagged superlatives require
removal or explicit evidence review.

Phase 1 voice input is synthetic transcript text and follows this boundary:

```text
synthetic transcript text -> DirectionV1 -> policy -> non executable preview
```

Phase 1 requests no microphone permission and accepts no audio. Future audio,
transcription, custom voice, consent, device enrollment, and live approval are
separate implementation gates. Voice is not a signature and cannot authorize
an external effect.

## Analytics and optimization

Each `NormalizedMetricsV1` snapshot binds platform, account, identity scope,
collection mode, connector definition version, content identifier and digest,
source and provenance digests, evidence label, time window, collection time,
quality flags, counters, definitions, and derived rate semantics. Synthetic
fixtures can be `SYNTHETIC`, platform exports can be `MEASURED`, and public
pages can be `MEASURED` or `CLAIMED`. Raw cross platform ranking is rejected
because platforms define impressions, reach, views, and engagement
differently. Aggregate engagements cannot be added to component counters,
click through rate uses exactly one click numerator, and potentially
overlapping comments and replies cannot be summed.

The default screening gate uses at least 20 paired observations and at least a
5 percent directional lift. Those values are Phase 1 floors and a triage rule,
not statistical significance. The experiment plan must be registered before
observation and must bind one registered normalized rate, its frozen semantics,
unique baseline and variant snapshot digests, the dataset, policy, pairing
rule, and anchor set. Anchor, provenance, security, and blocked action records
must be complete, digest bound, valid for no more than 30 days, current, and
declared passing. The harness does not verify the
authority of their issuers. A passing screen therefore produces
`SCREENING_PASSED_REQUIRES_INDEPENDENT_GATE_VERIFICATION`, with
`reviewEligibilityEstablished: false` and `promotionAuthorized: false`.

ADR 352 governs the implemented bounded proposal only autopilot. The CLI command
is `autopilot run`, and the read only MCP tool is `social_autopilot_run`. One
run accepts 1 through 100 proposals, a 1 through 100 cycle cap, and at most
20,000 aggregate observations within one exact account and identity scope. It
may resume from a structurally verified, run, batch, scope, and identity
registry bound checkpoint. Checkpoint issuer authority is not verified. Every
run reports `checkpointAuthorityVerified: false` and cannot mutate policy or
canonical learning, establish review eligibility, promote a candidate, call a
connector, or create an external effect.

## Deployment boundary

The optional GCP module is a Phase G1 static and proposed read only health and
capability service. Its design has internal ingress, an unprivileged service
account, scale to zero by default, a three instance ceiling, a full Artifact
Registry application image digest requirement, and an optional authenticated
Scheduler heartbeat. It grants no platform permissions and creates no secret
versions. No Terraform validation, reviewed live plan, container build, or GCP
deployment has been completed.

Continuous Discord connections, OAuth callbacks, platform webhooks, durable
queues, approval storage, and adapter specific service accounts are future
phases. They must be restartable and idempotent. See `deploy/gcp/README.md`.

## Release gate

Phase 1 is acceptable when tests and the deterministic closed package manifest
pass, the doctor scans every packaged file and finds no credential value or
capability link, all MCP tools remain read only, seven exact policy laundering
attacks are denied before network access, and receipt verification is anchored
to an externally retained expected head and count. The static import guard
covers every source module. An operating system network deny gate covers
selected suites that do not require the local HTTP test server; endpoint tests
remain separate.
Fresh Ruflo threat and MCP scans plus final diff review are still required, and
no unrelated worktree change may be included.

The path scoped workflow in `.github/workflows/social-metaharness.yml` repeats
the deterministic gates on Node 20 and Node 22 and runs the operating system
network denial profile on macOS. ADR 353 governs this CI evidence. The workflow
does not publish, deploy, connect an account, or make its checks mandatory in
repository branch protection.
