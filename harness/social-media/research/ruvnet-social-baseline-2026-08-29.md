# Ruvnet social identity and evidence baseline

Snapshot date: 29 August 2026

Research mode: public, unauthenticated, and read only

## Answer

The public evidence supports read only monitoring of the personal rUv or
ruvnet presence on GitHub, GitHub Gists, LinkedIn, and X. Instagram is retained
only as a historically associated observation target; current ownership,
activity, metrics, and control are unverified. Public research establishes no
write authority on any platform. Discord is an adjacent Agentics community
surface. WhatsApp is an adjacent legacy surface whose historical figures lack
retained source URLs. Reddit has third party discussion and an adjacent
organization claim but no verified personal account or owned property.
Facebook and Threads remain unresolved.

The strongest observable publishing surfaces are LinkedIn, GitHub, and GitHub
Gists. X has substantial indexed reach but limited public observability.
Discord appears to be the current community hub while WhatsApp appears to be a
legacy and fragmented community surface.

## Evidence contract

| Label | Meaning |
|---|---|
| `MEASURED` | Observed on a public page or public search index during this research. It is a dated snapshot, not a current guarantee. |
| `CLAIMED` | Published by the account, organization, or an associated official site but not independently reproduced. |
| `UNVERIFIED` | Ownership, current metric, or current activity could not be established from public evidence. |

Public profile links support attribution. They do not prove current
administrator control, credential scope, or permission to publish.

## Dated baseline

| Surface | Identity status | Public snapshot | Recent public themes | Grade and limitation |
|---|---|---|---|---|
| [GitHub ruvnet](https://github.com/ruvnet) | Strongly attributable | 11.2K followers, 53 following, 210 repositories. RuView had 91,822 stars, Ruflo 69,550, RuVector 4,459, and metaharness 616. | RF spatial sensing, agent orchestration, vector memory, and evidence gated metaharness work | `MEASURED` on 29 August 2026. Counters are volatile. |
| [GitHub Gists ruvnet](https://gist.github.com/ruvnet) | Strongly attributable | 480 gists, 13 starred gists, 11.2K followers. The latest visible gist was created on 27 August 2026. | Frontier research review, integration maps, architecture decisions, and evidence gated optimization | `MEASURED` on 29 August 2026. |
| [LinkedIn rUv Cohen](https://ca.linkedin.com/in/reuvencohen) | Strongly attributable | 62K followers and 500 plus connections. Multiple posts were visible from the preceding 24 hours. | RuView calibration, WorldGraph, Ruflo, building in public, and sensing privacy | `MEASURED` on 29 August 2026. The follower count is rounded. |
| [X rUv](https://x.com/ruv) | Strongly attributable through GitHub and [verified Gravatar](https://gravatar.com/ruvnet) links | About 54K followers and 112 following | No current theme recorded because recent posts were not publicly retrievable | `MEASURED` from indexed public metadata that was approximately two months old. Freshness is limited. |
| [Instagram ruv](https://www.instagram.com/ruv/) | Historical association only; current control is unverified | Current counters were unavailable | No current theme recorded | `UNVERIFIED` for current ownership, metrics, and activity. It is not current write authority. |
| Discord community | Adjacent Agentics Foundation surface | 3K plus members | Channel contents were not inspected | `CLAIMED` by the [Agentics Foundation](https://agentics.org/). No invite token or join URL is retained. |
| WhatsApp communities | Adjacent legacy Agentics surface | Historical statements mentioned 1,100 active users and later 1,200 users | Statements described migration toward Discord because of capacity, spam, and moderation limits | Both figures are `UNVERIFIED` because no exact source URL was retained. They are stale, excluded from machine metrics and publication evidence, and do not establish current membership or activity. No group identifier or join URL is retained. |
| Reddit | No strongly attributable personal account or owned property found | [Agentics Foundation](https://agentics.org/) claims 130K plus Reddit followers but the reviewed evidence did not identify a specific owned subreddit | Third party discussion exists | The 130K figure is an adjacent organization `CLAIMED` value. Property ownership is `UNVERIFIED`, and the figure is not personal rUv reach. |
| Facebook | No strongly attributable account found | None | None recorded | `UNVERIFIED`. A third party RuView coin listing is not treated as official. |
| Threads | No strongly attributable account found | None | None recorded | `UNVERIFIED`. Instagram handle ownership does not prove Threads ownership. |

## Adjacent identity boundary

| Identity | Public metric | Governance decision |
|---|---:|---|
| [Agentics Foundation LinkedIn](https://www.linkedin.com/company/agentics-org) | 6,974 followers, `MEASURED` on 29 August 2026 | Keep separate from personal rUv and ruvnet credentials, policy, analytics, and approvals. |
| [Cognitum One LinkedIn](https://www.linkedin.com/company/cognitum-one) | 388 followers, `MEASURED` on 29 August 2026 | Keep separate from personal rUv and ruvnet credentials, policy, analytics, and approvals. |

Audience counters overlap. They must not be summed as unique people.

## Inputs

1. Public GitHub and GitHub Gists profiles for direct identity, metric, and
   activity observation.

2. Public LinkedIn personal and organization profiles for identity, metric,
   and visible theme observation.

3. The verified Gravatar identity directory for the GitHub, LinkedIn, and X
   link chain.

4. Public X search index metadata because direct unauthenticated retrieval was
   restricted.

5. The public Agentics Foundation site for Discord and Reddit organization
   claims. These remain `CLAIMED` and separate from personal rUv reach.

6. Historical WhatsApp statements used only as context. Because no exact
   source URL was retained, their figures are `UNVERIFIED` and excluded from
   machine metrics and publication evidence.

## Outputs

1. A machine readable dated baseline in
   `harness/social-media/research/ruvnet-social-baseline-2026-08-29.json`.

2. This human readable research record.

3. ADR 345, which defines the identity and evidence boundary for later social
   media harness work.

## Assumptions

1. Platform counters are volatile and may be rounded.

2. Search index metadata can lag live state.

3. A public identity link does not prove current administrator control.

4. Personal rUv, ruvnet, Agentics Foundation, and Cognitum One require
   separate governance until an authorized owner explicitly maps them.

5. Private posts, private groups, direct messages, and authenticated analytics
   are outside this baseline.

## Decision

1. Permit read only monitoring of the strongly attributable surfaces.

2. Keep every write connector disabled until an authorized owner supplies the
   exact platform identifier, proves administrator control, approves the
   credential scope, and defines a human approval rule.

3. Treat Discord, WhatsApp, and Reddit as separate community identity mappings.
   Do not use the historical WhatsApp figures as metrics or publication
   evidence, and do not assign the adjacent Reddit claim to personal rUv reach.

4. Treat Facebook and Threads as unresolved and fail closed.

5. Require every quantitative publication to carry an evidence label, source,
   measurement date, and reproducer where possible.

## Risks and controls

| Risk | Severity | Control |
|---|---|---|
| Unsupported performance or adoption claims are amplified across platforms | High | Require an evidence receipt and named human approval before publication. |
| Personal, ruvnet, Agentics Foundation, and Cognitum One identities are conflated | High | Use separate tenant, credential, policy, analytics, and approval records. |
| Public counters drift | Medium | Store dated snapshots and never present an old value as current. |
| Third party Facebook, Reddit, or token related properties are mistaken for official properties | High | Fail closed until the exact platform identifier and administrator control are verified. |
| Community access credentials leak into source control | High | Store no invite token, group identifier, or join URL. |

The largest reputational risk is the credibility gap visible in third party
Reddit discussion around Ruflo and Claude Flow. Commenters repeatedly question
performance and adoption numbers, documentation quality, security, and
promotional volume. These comments are opinions, not verified findings. They
still identify the likely failure mode: broad automation can distribute a
claim faster than evidence can correct it.

## Acceptance test

1. Parse the JSON without error.

2. In a fresh unauthenticated browser, reproduce the GitHub, Gists, LinkedIn,
   and X identity link chain while allowing for counter drift and index lag.

3. Confirm that every machine metric is marked `MEASURED` or `CLAIMED` and
   includes a date or explicit freshness limitation. Confirm that the two
   unsourced historical WhatsApp figures are `UNVERIFIED` and excluded from
   machine metrics and publication evidence.

4. Confirm that Facebook, Threads, Reddit, Discord, and WhatsApp retain
   `NOT_ESTABLISHED` write authority.

5. Confirm that Agentics Foundation and Cognitum One remain separate identity
   records.

6. Confirm that no live Discord or WhatsApp invite token, group identifier, or
   join URL appears in either baseline artifact or ADR 345.

Acceptance is `PASS` only when all six checks pass.
