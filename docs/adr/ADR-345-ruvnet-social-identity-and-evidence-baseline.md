# ADR-345: Ruvnet social identity and evidence baseline

## Status

Proposed. Public baseline research is complete. Account administrator control,
write credentials, and platform policy qualification remain unverified.

## Context

A social media metaharness can create business value through consistent
publishing, faster response, and measured content improvement. It can also
amplify an unsupported claim, publish under the wrong organization, expose a
community access credential, or violate platform policy within minutes.

The public rUv ecosystem spans a personal identity, ruvnet code properties,
Agentics Foundation communities, and Cognitum One. Similar names and shared
projects are not sufficient authority to merge those identities. Public links
can support attribution but cannot prove administrator control or permission
to write.

The baseline must therefore separate identity evidence, public metrics, write
authority, and adjacent organization governance before any API, MCP, browser,
computer use, or application connector is enabled.

## Inputs

1. Public unauthenticated GitHub, GitHub Gists, LinkedIn, X, Instagram,
   Agentics Foundation, and Cognitum One pages.

2. Verified public identity links connecting the ruvnet GitHub profile,
   LinkedIn personal profile, and X profile.

3. Public search index metadata where direct unauthenticated platform access
   was restricted.

4. Public organization statements about Discord and Reddit. These are
   organization claims rather than independent measurements and do not prove
   ownership of a Reddit property.

5. Historical WhatsApp statements used only as context. Their exact source
   URLs were not retained, so the figures are `UNVERIFIED` and excluded from
   machine metrics and publication evidence.

6. Third party discussion used only for reputation risk discovery. Third party
   discussion is not identity authority and does not verify product claims.

Private groups, direct messages, authenticated analytics, credential stores,
and account settings are excluded.

## Outputs

1. `harness/social-media/research/ruvnet-social-baseline-2026-08-29.json`
   provides a machine readable dated identity and metric snapshot.

2. `harness/social-media/research/ruvnet-social-baseline-2026-08-29.md`
   provides the human readable evidence record and limitations.

3. This decision defines the identity and evidence gate for later harness
   architecture, implementation, validation, security review, and deployment.

## Assumptions

1. Public metrics are volatile, sometimes rounded, and not unique audience
   counts.

2. Search index metadata can lag the live platform.

3. Account links support attribution but do not establish current write
   authority.

4. Agentics Foundation and Cognitum One have independent governance until an
   authorized owner explicitly proves otherwise.

5. Discord and WhatsApp access material is sensitive capability data even when
   a public page exposes it.

## Decision

The dated baseline is the initial identity registry for the proposed Ruvnet
social media metaharness.

The harness may use strongly attributable GitHub, GitHub Gists, LinkedIn, and X
identities for public read only monitoring. Instagram is retained only as a
historically associated observation target. Its current ownership, activity,
metrics, and control are `UNVERIFIED`. Public research grants no write
authority.

Each write enabled platform adapter must remain disabled until all of the
following evidence exists:

1. The exact platform account, page, organization, or community identifier.

2. Proof that the operator controls that property as an administrator.

3. A least authority credential scope and documented revocation path.

4. A platform policy and rate limit qualification.

5. A named human approval rule for publishing, replying, deleting, messaging,
   spending, or changing account state.

Personal rUv, ruvnet, Agentics Foundation, and Cognitum One must be separate
tenants with separate credentials, policies, analytics, content queues, and
approvals. Shared content requires an explicit distribution plan rather than
implicit account reuse.

Discord, WhatsApp, and Reddit require separate community owner mapping. The
historical WhatsApp figures cannot be used as metrics or publication evidence.
The Agentics Reddit figure remains an adjacent organization `CLAIMED` value
with property ownership `UNVERIFIED`; it is not personal rUv reach. Facebook
and Threads fail closed until exact properties and administrator control are
verified. Instagram ownership does not prove Threads ownership.

Every quantitative publication must carry an evidence receipt containing the
claim label, source, observation date, metric definition, and reproducer where
possible. The permitted labels for this baseline are `MEASURED`, `CLAIMED`,
and `UNVERIFIED`. A stale measurement cannot be restated as current. Platform
audience counters must never be summed as unique audience.

No Discord or WhatsApp invite token, group identifier, or join URL may be
stored in tracked research, configuration, logs, prompts, analytics, or ADRs.

## Risks and controls

| Risk | Severity | Control |
|---|---|---|
| Unsupported performance or adoption claims are amplified | High | Require an evidence receipt and named human approval before publication. |
| Content is published under the wrong identity | High | Separate tenant, credential, queue, policy, and approval records. |
| Public counters drift or are rounded | Medium | Store dated snapshots and enforce freshness limits. |
| Unsourced historical WhatsApp figures enter machine analytics or publication | High | Label them `UNVERIFIED` and exclude them from machine metrics and publication evidence. |
| An adjacent Reddit claim is assigned to personal rUv reach | High | Preserve the organization source, `CLAIMED` label, and `UNVERIFIED` property ownership. |
| An impersonation or token related property is treated as official | High | Fail closed until exact identifier and administrator control are verified. |
| Community access capability leaks into source control or model context | High | Prohibit invite tokens, group identifiers, and join URLs in tracked artifacts and logs. |
| Browser or computer use bypasses API governance | High | Apply the same policy engine, approval gate, audit receipt, and rate limit to every transport. |
| Self optimization promotes engagement at the expense of evidence quality | High | Optimize only within reviewed objectives and require holdout quality, safety, and trust gates before promotion. |

The primary failure mode is reputational rather than technical. Third party
Reddit discussion contains recurring skepticism about adoption and performance
numbers, documentation, security, and promotional volume. These are user
opinions, not verified findings. They are still useful risk evidence. A social
harness that maximizes reach without an evidence gate would increase this risk.

## Consequences

The near term cost is one identity record, one credential boundary, and one
approval policy per platform and organization. This adds configuration and
operator review latency. It reduces the materially larger risk of unauthorized
publishing, identity confusion, credential leakage, and automated credibility
damage.

Read only monitoring can begin after source and rate limit qualification.
Publishing, replies, direct messages, moderation, deletion, account changes,
and spending remain blocked until the write evidence gate is satisfied.

The dated metrics are suitable for baseline comparison. They are not suitable
as permanent marketing claims or unique audience totals.

## Acceptance test

Acceptance requires all of the following:

1. The JSON baseline parses without error.

2. A fresh unauthenticated browser reproduces the GitHub, Gists, LinkedIn, and
   X identity link chain, allowing for documented counter drift and index lag.

3. Every machine metric is labeled `MEASURED` or `CLAIMED` and has an
   observation date or explicit freshness limitation. The two unsourced
   historical WhatsApp figures remain `UNVERIFIED` and excluded from machine
   metrics and publication evidence.

4. Every platform has `NOT_ESTABLISHED` write authority until owner supplied
   account identifiers and administrator evidence are reviewed.

5. Agentics Foundation and Cognitum One remain separate from personal rUv and
   ruvnet records.

6. A repository search finds no live Discord or WhatsApp invite token, group
   identifier, or join URL in the two baseline artifacts or this ADR.

7. Instagram, Facebook, Threads, Reddit, Discord, and WhatsApp fail closed for
   writes until their exact current property and owner evidence is reviewed.

8. The adjacent Agentics Reddit claim remains separate from personal rUv reach
   and records property ownership as `UNVERIFIED`.

The decision is accepted only when all eight checks pass.
